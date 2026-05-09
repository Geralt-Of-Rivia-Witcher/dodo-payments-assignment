use axum::{
    body::{to_bytes, Body},
    http::Request,
    routing::post,
    Json, Router,
};
use invoice_api::{auth::AppState, build_app};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, Row};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn psp_timeout_does_not_leave_invoice_in_bad_state() {
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

    let psp_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind psp listener");
    let psp_addr = psp_listener.local_addr().expect("psp local addr");

    let psp_app = Router::new().route(
        "/charges",
        post(|| async {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            Json(json!({
                "status": "succeeded",
                "psp_ref": Uuid::new_v4().to_string()
            }))
        }),
    );

    tokio::spawn(async move {
        axum::serve(psp_listener, psp_app)
            .await
            .expect("serve test psp");
    });

    let state = AppState {
        db: pool.clone(),
        psp_base_url: format!("http://{}", psp_addr),
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(400))
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
        .bind("PSP Failure Test Customer")
        .bind(format!("psp-failure-{}@example.com", Uuid::new_v4()))
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
    .bind(2500_i64)
    .execute(&pool)
    .await
    .expect("insert invoice");

    let request = Request::builder()
        .method("POST")
        .uri(format!("/invoices/{invoice_id}/pay"))
        .header("content-type", "application/json")
        .header("authorization", "Bearer dodo_test_live_key_1234567890")
        .header(
            "idempotency-key",
            format!("psp-timeout-key-{}", Uuid::new_v4()),
        )
        .body(Body::from(
            serde_json::to_vec(&json!({ "card_token": "tok_timeout" })).expect("serialize body"),
        ))
        .expect("build request");

    let response = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        app.clone().oneshot(request),
    )
    .await
    .expect("pay request should not hang")
    .expect("pay response");

    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read response body");
    let json_body: serde_json::Value = serde_json::from_slice(&body).expect("parse response body");

    assert_eq!(
        json_body.get("status").and_then(|v| v.as_str()),
        Some("failed"),
        "timeout case should result in failed payment attempt"
    );
    assert_eq!(
        json_body.get("failure_code").and_then(|v| v.as_str()),
        Some("psp_timeout"),
        "timeout case should map to psp_timeout failure code"
    );

    let final_state: String = sqlx::query("SELECT state FROM invoices WHERE id = $1")
        .bind(invoice_id)
        .fetch_one(&pool)
        .await
        .expect("read final invoice state")
        .get("state");

    assert_eq!(
        final_state, "open",
        "invoice should return to open state after PSP timeout"
    );

    let failed_attempts: i64 = sqlx::query(
        "SELECT COUNT(*)::BIGINT AS count FROM payment_attempts WHERE invoice_id = $1 AND status = 'failed'",
    )
    .bind(invoice_id)
    .fetch_one(&pool)
    .await
    .expect("count failed attempts")
    .get("count");

    assert_eq!(
        failed_attempts, 1,
        "expected exactly one failed payment attempt"
    );
}
