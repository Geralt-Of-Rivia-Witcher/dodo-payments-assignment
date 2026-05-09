use axum::{
    extract::{Extension, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    auth::{AppState, AuthBusiness},
    error::ApiError,
};

#[derive(Debug, Deserialize)]
pub struct CreateWebhookEndpointRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct WebhookEndpointResponse {
    pub id: Uuid,
    pub url: String,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_webhook_endpoint(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthBusiness>,
    Json(req): Json<CreateWebhookEndpointRequest>,
) -> Result<Json<WebhookEndpointResponse>, ApiError> {
    let url = req.url.trim();

    if url.is_empty() {
        return Err(ApiError::bad_request("validation_error", "url is required"));
    }

    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(ApiError::bad_request(
            "validation_error",
            "url must start with http:// or https://",
        ));
    }

    let id = Uuid::new_v4();
    let signing_secret = format!("whsec_{}", Uuid::new_v4());

    let row = sqlx::query(
        "INSERT INTO webhook_endpoints (id, business_id, url, signing_secret, is_active) VALUES ($1, $2, $3, $4, $5) RETURNING id, url, is_active, created_at",
    )
    .bind(id)
    .bind(auth.business_id)
    .bind(url)
    .bind(signing_secret)
    .bind(true)
    .fetch_one(&state.db)
    .await
    .map_err(|_| ApiError::internal("failed to create webhook endpoint"))?;

    Ok(Json(WebhookEndpointResponse {
        id: row.get("id"),
        url: row.get("url"),
        is_active: row.get("is_active"),
        created_at: row.get("created_at"),
    }))
}

pub async fn list_webhook_endpoints(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthBusiness>,
) -> Result<Json<Vec<WebhookEndpointResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, url, is_active, created_at FROM webhook_endpoints WHERE business_id = $1 ORDER BY created_at DESC",
    )
    .bind(auth.business_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| ApiError::internal("failed to list webhook endpoints"))?;

    let endpoints = rows
        .into_iter()
        .map(|row| WebhookEndpointResponse {
            id: row.get("id"),
            url: row.get("url"),
            is_active: row.get("is_active"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(Json(endpoints))
}
