use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct PspChargeRequest {
    card_token: String,
    amount_cents: i64,
}

#[derive(Debug, Serialize)]
struct PspSuccessResponse {
    status: &'static str,
    psp_ref: String,
}

#[derive(Debug, Serialize)]
struct PspFailureResponse {
    status: &'static str,
    code: &'static str,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/charges", post(create_charge));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081")
        .await
        .expect("bind mock-psp");

    info!("mock-psp listening on 0.0.0.0:8081");
    axum::serve(listener, app).await.expect("serve mock-psp");
}

async fn create_charge(Json(req): Json<PspChargeRequest>) -> impl IntoResponse {
    if req.amount_cents < 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "failed",
                "code": "invalid_amount"
            })),
        )
            .into_response();
    }

    match req.card_token.as_str() {
        "tok_success" => {
            sleep(Duration::from_millis(100)).await;
            (
                StatusCode::OK,
                Json(serde_json::to_value(PspSuccessResponse {
                    status: "succeeded",
                    psp_ref: Uuid::new_v4().to_string(),
                })
                .expect("serialize success response")),
            )
                .into_response()
        }
        "tok_insufficient_funds" => {
            sleep(Duration::from_millis(100)).await;
            (
                StatusCode::OK,
                Json(serde_json::to_value(PspFailureResponse {
                    status: "failed",
                    code: "insufficient_funds",
                })
                .expect("serialize failure response")),
            )
                .into_response()
        }
        "tok_card_declined" => {
            sleep(Duration::from_millis(100)).await;
            (
                StatusCode::OK,
                Json(serde_json::to_value(PspFailureResponse {
                    status: "failed",
                    code: "card_declined",
                })
                .expect("serialize failure response")),
            )
                .into_response()
        }
        "tok_timeout" => {
            sleep(Duration::from_secs(30)).await;
            (
                StatusCode::OK,
                Json(serde_json::to_value(PspSuccessResponse {
                    status: "succeeded",
                    psp_ref: Uuid::new_v4().to_string(),
                })
                .expect("serialize success response")),
            )
                .into_response()
        }
        "tok_network_error" => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        _ => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "failed",
                "code": "unknown_token"
            })),
        )
            .into_response(),
    }
}
