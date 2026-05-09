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
    pub payment_attempt_id: Uuid,
    pub status: &'static str,
    pub message: &'static str,
    pub idempotent_replay: bool,
}

pub async fn pay_invoice(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthBusiness>,
    Path(invoice_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<PayInvoiceRequest>,
) -> Result<Json<PayInvoiceResponse>, ApiError> {
    let idempotency_key = read_idempotency_key(&headers)?;
    let card_token = req.card_token.trim().to_string();

    if card_token.is_empty() {
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

    let existing_attempt = sqlx::query(
        "SELECT id, invoice_id, card_token FROM payment_attempts WHERE business_id = $1 AND idempotency_key = $2",
    )
    .bind(auth.business_id)
    .bind(&idempotency_key)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| ApiError::internal("failed to check idempotency key"))?;

    if let Some(row) = existing_attempt {
        return build_replay_or_conflict(row, invoice_id, &card_token);
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::internal("failed to start payment transaction"))?;

    let invoice =
        sqlx::query("SELECT id, state FROM invoices WHERE id = $1 AND business_id = $2 FOR UPDATE")
            .bind(invoice_id)
            .bind(auth.business_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| ApiError::internal("failed to fetch invoice"))?;

    let Some(invoice_row) = invoice else {
        return Err(ApiError::not_found("invoice not found"));
    };

    let current_state: String = invoice_row.get("state");

    if current_state != STATE_OPEN {
        if is_terminal_state(&current_state) {
            return Err(ApiError::conflict(
                "invoice_not_payable",
                format!("invoice is in terminal state '{current_state}'"),
            ));
        }

        return Err(ApiError::conflict(
            "invalid_state_transition",
            format!("cannot pay invoice in state '{current_state}'"),
        ));
    }

    let payment_attempt_id = Uuid::new_v4();

    let insert_result = sqlx::query(
        "INSERT INTO payment_attempts (id, business_id, invoice_id, idempotency_key, card_token, status) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(payment_attempt_id)
    .bind(auth.business_id)
    .bind(invoice_id)
    .bind(&idempotency_key)
    .bind(&card_token)
    .bind("pending")
    .execute(&mut *tx)
    .await;

    match insert_result {
        Ok(_) => {}
        Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("23505") => {
            tx.rollback()
                .await
                .map_err(|_| ApiError::internal("failed to rollback payment transaction"))?;

            let existing_attempt = sqlx::query(
                "SELECT id, invoice_id, card_token FROM payment_attempts WHERE business_id = $1 AND idempotency_key = $2",
            )
            .bind(auth.business_id)
            .bind(&idempotency_key)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| ApiError::internal("failed to resolve idempotency race"))?;

            let Some(row) = existing_attempt else {
                return Err(ApiError::internal(
                    "idempotency race detected but existing attempt not found",
                ));
            };

            return build_replay_or_conflict(row, invoice_id, &card_token);
        }
        Err(_) => {
            return Err(ApiError::internal("failed to create payment attempt"));
        }
    }

    tx.commit()
        .await
        .map_err(|_| ApiError::internal("failed to commit payment transaction"))?;

    Ok(Json(PayInvoiceResponse {
        invoice_id,
        payment_attempt_id,
        status: "accepted",
        message: "payment attempt recorded",
        idempotent_replay: false,
    }))
}

fn build_replay_or_conflict(
    row: sqlx::postgres::PgRow,
    invoice_id: Uuid,
    card_token: &str,
) -> Result<Json<PayInvoiceResponse>, ApiError> {
    let existing_attempt_id: Uuid = row.get("id");
    let existing_invoice_id: Uuid = row.get("invoice_id");
    let existing_card_token: String = row.get("card_token");

    if existing_invoice_id != invoice_id || existing_card_token != card_token {
        return Err(ApiError::conflict(
            "idempotency_conflict",
            "idempotency key was already used with a different request",
        ));
    }

    Ok(Json(PayInvoiceResponse {
        invoice_id,
        payment_attempt_id: existing_attempt_id,
        status: "accepted",
        message: "idempotent replay; existing payment attempt returned",
        idempotent_replay: true,
    }))
}

fn read_idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let value = headers.get("Idempotency-Key").ok_or_else(|| {
        ApiError::bad_request(
            "missing_idempotency_key",
            "Idempotency-Key header is required",
        )
    })?;

    let value = value.to_str().map_err(|_| {
        ApiError::bad_request(
            "invalid_idempotency_key",
            "Idempotency-Key must be valid text",
        )
    })?;

    Ok(value.to_string())
}
