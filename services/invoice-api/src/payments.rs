use axum::{
    extract::{Extension, Path, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    auth::{AppState, AuthBusiness},
    error::ApiError,
    invoice_state::{is_terminal_state, STATE_OPEN},
};

#[derive(Debug, Deserialize)]
pub struct PayInvoiceRequest {
    pub card_token: String,
}

#[derive(Debug, Serialize)]
pub struct PayInvoiceResponse {
    pub invoice_id: Uuid,
    pub status: &'static str,
    pub message: &'static str,
}

pub async fn pay_invoice(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthBusiness>,
    Path(invoice_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<PayInvoiceRequest>,
) -> Result<Json<PayInvoiceResponse>, ApiError> {
    let idempotency_key = read_idempotency_key(&headers)?;

    if req.card_token.trim().is_empty() {
        return Err(ApiError::bad_request(
            "validation_error",
            "card_token is required",
        ));
    }

    if idempotency_key.trim().is_empty() {
        return Err(ApiError::bad_request(
            "validation_error",
            "Idempotency-Key cannot be empty",
        ));
    }

    let invoice = sqlx::query("SELECT state FROM invoices WHERE id = $1 AND business_id = $2")
        .bind(invoice_id)
        .bind(auth.business_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| ApiError::internal("failed to fetch invoice"))?;

    let Some(invoice_row) = invoice else {
        return Err(ApiError::not_found("invoice not found"));
    };

    let state: String = invoice_row.get("state");

    if state != STATE_OPEN {
        if is_terminal_state(&state) {
            return Err(ApiError::conflict(
                "invoice_not_payable",
                format!("invoice is in terminal state '{state}'"),
            ));
        }

        return Err(ApiError::conflict(
            "invalid_state_transition",
            format!("cannot pay invoice in state '{state}'"),
        ));
    }

    Ok(Json(PayInvoiceResponse {
        invoice_id,
        status: "accepted",
        message: "payment skeleton validated; processing flow will be implemented later",
    }))
}

fn read_idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let value = headers
        .get("Idempotency-Key")
        .ok_or_else(|| ApiError::bad_request("missing_idempotency_key", "Idempotency-Key header is required"))?;

    let value = value
        .to_str()
        .map_err(|_| ApiError::bad_request("invalid_idempotency_key", "Idempotency-Key must be valid text"))?;

    Ok(value.to_string())
}
