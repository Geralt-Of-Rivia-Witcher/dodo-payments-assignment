use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::error::ApiError;

pub async fn enqueue_webhook_event_with_deliveries(
    tx: &mut Transaction<'_, Postgres>,
    business_id: Uuid,
    invoice_id: Uuid,
    event_type: &str,
    payload: serde_json::Value,
) -> Result<(), ApiError> {
    let event_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO webhook_events (id, business_id, invoice_id, event_type, payload_json) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(event_id)
    .bind(business_id)
    .bind(invoice_id)
    .bind(event_type)
    .bind(payload)
    .execute(&mut **tx)
    .await
    .map_err(|_| ApiError::internal("failed to enqueue webhook event"))?;

    let endpoint_rows =
        sqlx::query("SELECT id FROM webhook_endpoints WHERE business_id = $1 AND is_active = TRUE")
            .bind(business_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(|_| ApiError::internal("failed to load webhook endpoints"))?;

    for row in endpoint_rows {
        let endpoint_id: Uuid = row.get("id");

        sqlx::query(
            "INSERT INTO webhook_deliveries (id, event_id, endpoint_id, attempt_number, status, next_attempt_at) VALUES ($1, $2, $3, $4, $5, now())",
        )
        .bind(Uuid::new_v4())
        .bind(event_id)
        .bind(endpoint_id)
        .bind(0_i32)
        .bind("pending")
        .execute(&mut **tx)
        .await
        .map_err(|_| ApiError::internal("failed to enqueue webhook delivery"))?;
    }

    Ok(())
}
