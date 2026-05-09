use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::Request,
    routing::post,
    Json, Router,
};
use futures::future::join_all;
use invoice_api::{auth::AppState, build_app};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, Row};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone)]
struct PspTestState {
    calls: Arc<AtomicUsize>,
}

async fn psp_charge_handler(State(state): State<PspTestState>) -> Json<serde_json::Value> {
    state.calls.fetch_add(1, Ordering::SeqCst);
    Json(json!({
        "status": "succeeded",
        "psp_ref": Uuid::new_v4().to_string()
    }))
}

#[tokio::test]
async fn concurrent_pay_requests_allow_at_most_one_success_and_consistent_final_state() {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("connect test db");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("run migrations");

    let psp_calls = Arc::new(AtomicUsize::new(0));
    let psp_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind psp listener");
    let psp_addr = psp_listener.local_addr().expect("psp local addr");

    let psp_app = Router::new()
        .route("/charges", post(psp_charge_handler))
        .with_state(PspTestState {
            calls: psp_calls.clone(),
        });

    tokio::spawn(async move {
        axum::serve(psp_listener, psp_app)
            .await
            .expect("serve test psp");
    });

    let state = AppState {
        db: pool.clone(),
        psp_base_url: format!("http://{}", psp_addr),
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .expect("build http client"),
    };

    let app = build_app(state);

    let business_id =
        Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("seed business id");
    let customer_id = Uuid::new_v4();
    let invoice_id = Uuid::new_v4();

    sqlx::query("INSERT INTO customers (id, business_id, name, email) VALUES ($1, $2, $3, $4)")
        .bind(customer_id)
        .bind(business_id)
        .bind("Load Test Customer")
        .bind(format!("concurrency-{}@example.com", Uuid::new_v4()))
        .execute(&pool)
        .await
        .expect("insert customer");

    sqlx::query(
        "INSERT INTO invoices (id, business_id, customer_id, state, total_amount_cents, due_date) VALUES ($1, $2, $3, $4, $5, CURRENT_DATE)",
    )
    .bind(invoice_id)
    .bind(business_id)
    .bind(customer_id)
    .bind("open")
    .bind(1000_i64)
    .execute(&pool)
    .await
    .expect("insert invoice");

    let n = 10;
    let mut tasks = Vec::with_capacity(n);

    for i in 0..n {
        let app_clone = app.clone();
        let request = Request::builder()
            .method("POST")
            .uri(format!("/invoices/{invoice_id}/pay"))
            .header("content-type", "application/json")
            .header("authorization", "Bearer dodo_test_live_key_1234567890")
            .header(
                "idempotency-key",
                format!("concurrency-key-{i}-{}", Uuid::new_v4()),
            )
            .body(Body::from(
                serde_json::to_vec(&json!({ "card_token": "tok_success" }))
                    .expect("serialize body"),
            ))
            .expect("build request");

        tasks.push(tokio::spawn(async move {
            app_clone.oneshot(request).await.expect("oneshot response")
        }));
    }

    let responses = join_all(tasks)
        .await
        .into_iter()
        .map(|r| r.expect("task join"))
        .collect::<Vec<_>>();

    let mut succeeded_responses = 0_usize;
    for resp in responses {
        let bytes = to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("read response body");
        let parsed: serde_json::Value =
            serde_json::from_slice(&bytes).expect("parse response json");
        if parsed
            .get("status")
            .and_then(|s| s.as_str())
            .map(|s| s == "succeeded")
            .unwrap_or(false)
        {
            succeeded_responses += 1;
        }
    }

    assert_eq!(
        succeeded_responses, 1,
        "expected exactly one response with status=succeeded, got {succeeded_responses}"
    );

    let psp_call_count = psp_calls.load(Ordering::SeqCst);
    assert_eq!(
        psp_call_count, 1,
        "expected exactly one PSP /charges call, got {psp_call_count}"
    );

    let succeeded_attempts: i64 = sqlx::query(
        "SELECT COUNT(*)::BIGINT AS count FROM payment_attempts WHERE invoice_id = $1 AND status = 'succeeded'",
    )
    .bind(invoice_id)
    .fetch_one(&pool)
    .await
    .expect("count succeeded attempts")
    .get("count");

    assert_eq!(
        succeeded_attempts, 1,
        "expected exactly one succeeded payment attempt, got {succeeded_attempts}"
    );

    let final_state: String = sqlx::query("SELECT state FROM invoices WHERE id = $1")
        .bind(invoice_id)
        .fetch_one(&pool)
        .await
        .expect("read final invoice state")
        .get("state");

    assert_eq!(
        final_state, "paid",
        "expected final invoice state to be paid"
    );
}
