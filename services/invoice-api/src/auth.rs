use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

#[derive(Clone, Copy)]
pub struct AuthBusiness {
    pub business_id: Uuid,
}

pub async fn require_api_key(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let raw_key = extract_bearer_token(&req)?;
    let prefix = key_prefix(raw_key);
    let key_hash = hash_key(raw_key);

    let row = sqlx::query_as::<_, (Uuid, String, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT business_id, key_hash, revoked_at FROM api_keys WHERE key_prefix = $1 LIMIT 1",
    )
    .bind(prefix)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| ApiError::internal("failed to query api keys"))?;

    let (business_id, stored_hash, revoked_at) = match row {
        Some(v) => v,
        None => return Err(ApiError::unauthorized("invalid api key")),
    };

    if revoked_at.is_some() {
        return Err(ApiError::unauthorized("api key revoked"));
    }

    if stored_hash != key_hash {
        return Err(ApiError::unauthorized("invalid api key"));
    }

    req.extensions_mut().insert(AuthBusiness { business_id });
    Ok(next.run(req).await)
}

fn extract_bearer_token(req: &Request) -> Result<&str, ApiError> {
    let value = req
        .headers()
        .get(header::AUTHORIZATION)
        .ok_or_else(|| ApiError::unauthorized("missing authorization header"))?;

    let auth = value
        .to_str()
        .map_err(|_| ApiError::unauthorized("invalid authorization header"))?;

    let token = auth
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::unauthorized("expected bearer token"))?;

    if token.is_empty() {
        return Err(ApiError::unauthorized("empty api key"));
    }

    Ok(token)
}

fn key_prefix(raw_key: &str) -> String {
    raw_key.chars().take(9).collect()
}

fn hash_key(raw_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    let digest = hasher.finalize();
    hex::encode(digest)
}
