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
async fn same_idempotency_key_returns_same_response_and_calls_psp_once() {
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
        .bind("Idempotency Test Customer")
        .bind(format!("idempotency-{}@example.com", Uuid::new_v4()))
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
    .bind(1500_i64)
    .execute(&pool)
    .await
    .expect("insert invoice");

    let idempotency_key = format!("idem-key-{}", Uuid::new_v4());

    let make_request = || {
        Request::builder()
            .method("POST")
            .uri(format!("/invoices/{invoice_id}/pay"))
            .header("content-type", "application/json")
            .header("authorization", "Bearer dodo_test_live_key_1234567890")
            .header("idempotency-key", &idempotency_key)
            .body(Body::from(
                serde_json::to_vec(&json!({ "card_token": "tok_success" }))
                    .expect("serialize body"),
            ))
            .expect("build request")
    };

    let resp1 = app
        .clone()
        .oneshot(make_request())
        .await
        .expect("first response");
    let body1 = to_bytes(resp1.into_body(), 1024 * 1024)
        .await
        .expect("read first body");
    let json1: serde_json::Value = serde_json::from_slice(&body1).expect("parse first body");

    let resp2 = app
        .clone()
        .oneshot(make_request())
        .await
        .expect("second response");
    let body2 = to_bytes(resp2.into_body(), 1024 * 1024)
        .await
        .expect("read second body");
    let json2: serde_json::Value = serde_json::from_slice(&body2).expect("parse second body");

    assert_eq!(
        json1.get("invoice_id"),
        json2.get("invoice_id"),
        "invoice_id should be stable across idempotent retries"
    );
    assert_eq!(
        json1.get("payment_attempt_id"),
        json2.get("payment_attempt_id"),
        "payment_attempt_id should be stable across idempotent retries"
    );
    assert_eq!(
        json1.get("status"),
        json2.get("status"),
        "status should be stable across idempotent retries"
    );
    assert_eq!(
        json1.get("idempotent_replay").and_then(|v| v.as_bool()),
        Some(false),
        "first response should not be marked as replay"
    );
    assert_eq!(
        json2.get("idempotent_replay").and_then(|v| v.as_bool()),
        Some(true),
        "second response should be marked as idempotent replay"
    );

    let psp_call_count = psp_calls.load(Ordering::SeqCst);
    assert_eq!(
        psp_call_count, 1,
        "expected exactly one PSP /charges call, got {psp_call_count}"
    );

    let attempt_count: i64 = sqlx::query(
        "SELECT COUNT(*)::BIGINT AS count FROM payment_attempts WHERE business_id = $1 AND idempotency_key = $2",
    )
    .bind(business_id)
    .bind(&idempotency_key)
    .fetch_one(&pool)
    .await
    .expect("count attempts by idempotency key")
    .get("count");

    assert_eq!(
        attempt_count, 1,
        "expected one payment_attempt row for the idempotency key"
    );
}
