use chrono::Utc;
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use sqlx::Row;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::auth::AppState;

type HmacSha256 = Hmac<Sha256>;

const MAX_ATTEMPTS: i32 = 5;

pub fn spawn_webhook_worker(state: AppState) {
    tokio::spawn(async move {
        info!("webhook worker started");

        loop {
            if let Err(err) = process_once(&state).await {
                error!(error = %err, "webhook worker iteration failed");
            }
            sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn process_once(state: &AppState) -> Result<(), String> {
    loop {
        let claimed = claim_next_delivery(state).await?;
        let Some(delivery) = claimed else {
            break;
        };

        let payload_text = serde_json::to_string(&delivery.payload)
            .map_err(|_| "failed to serialize webhook payload".to_string())?;

        let timestamp = Utc::now().timestamp().to_string();
        let signature = sign_payload(
            &delivery.signing_secret,
            &timestamp,
            &delivery.event_id.to_string(),
            &payload_text,
        )
        .map_err(|_| "failed to sign webhook payload".to_string())?;

        let response = state
            .http_client
            .post(&delivery.url)
            .header("Content-Type", "application/json")
            .header("X-Dodo-Event-Type", &delivery.event_type)
            .header("X-Dodo-Event-Id", delivery.event_id.to_string())
            .header("X-Dodo-Timestamp", &timestamp)
            .header("X-Dodo-Signature", signature)
            .body(payload_text)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                info!(
                    delivery_id = %delivery.delivery_id,
                    event_id = %delivery.event_id,
                    attempt_number = delivery.attempt_number + 1,
                    http_status = resp.status().as_u16(),
                    "webhook delivery succeeded"
                );
                sqlx::query(
                    "UPDATE webhook_deliveries
                     SET status = 'succeeded',
                         attempt_number = $1,
                         last_http_status = $2,
                         last_error = NULL,
                         next_attempt_at = NULL,
                         updated_at = now()
                     WHERE id = $3",
                )
                .bind(delivery.attempt_number + 1)
                .bind(resp.status().as_u16() as i32)
                .bind(delivery.delivery_id)
                .execute(&state.db)
                .await
                .map_err(|_| "failed to mark webhook delivery as succeeded".to_string())?;
            }
            Ok(resp) => {
                let status_code = resp.status().as_u16() as i32;
                warn!(
                    delivery_id = %delivery.delivery_id,
                    event_id = %delivery.event_id,
                    attempt_number = delivery.attempt_number + 1,
                    http_status = status_code,
                    "webhook delivery failed with non-2xx response"
                );
                handle_delivery_failure(
                    state,
                    delivery.delivery_id,
                    delivery.attempt_number,
                    Some(status_code),
                    format!("http_status_{status_code}"),
                )
                .await?;
            }
            Err(err) => {
                let error_label = if err.is_timeout() {
                    "timeout".to_string()
                } else {
                    "network_error".to_string()
                };
                warn!(
                    delivery_id = %delivery.delivery_id,
                    event_id = %delivery.event_id,
                    attempt_number = delivery.attempt_number + 1,
                    error = %error_label,
                    "webhook delivery request failed"
                );
                handle_delivery_failure(
                    state,
                    delivery.delivery_id,
                    delivery.attempt_number,
                    None,
                    error_label,
                )
                .await?;
            }
        }
    }

    Ok(())
}

struct ClaimedDelivery {
    delivery_id: Uuid,
    attempt_number: i32,
    event_id: Uuid,
    event_type: String,
    payload: Value,
    url: String,
    signing_secret: String,
}

async fn claim_next_delivery(state: &AppState) -> Result<Option<ClaimedDelivery>, String> {
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| "failed to start claim transaction".to_string())?;

    let row = sqlx::query(
        "SELECT wd.id AS delivery_id, wd.attempt_number, wd.event_id,
                we.event_type, we.payload_json,
                ep.url, ep.signing_secret
         FROM webhook_deliveries wd
         JOIN webhook_events we ON we.id = wd.event_id
         JOIN webhook_endpoints ep ON ep.id = wd.endpoint_id
         WHERE wd.status = 'pending'
           AND (wd.next_attempt_at IS NULL OR wd.next_attempt_at <= now())
           AND ep.is_active = TRUE
         ORDER BY wd.created_at ASC
         LIMIT 1
         FOR UPDATE SKIP LOCKED",
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| "failed to claim webhook delivery".to_string())?;

    let Some(row) = row else {
        tx.commit()
            .await
            .map_err(|_| "failed to commit empty claim transaction".to_string())?;
        return Ok(None);
    };

    let delivery_id: Uuid = row.get("delivery_id");
    let attempt_number: i32 = row.get("attempt_number");

    sqlx::query(
        "UPDATE webhook_deliveries
         SET next_attempt_at = now() + interval '15 seconds',
             updated_at = now()
         WHERE id = $1",
    )
    .bind(delivery_id)
    .execute(&mut *tx)
    .await
    .map_err(|_| "failed to lock-in claimed webhook delivery".to_string())?;

    tx.commit()
        .await
        .map_err(|_| "failed to commit claim transaction".to_string())?;

    Ok(Some(ClaimedDelivery {
        delivery_id,
        attempt_number,
        event_id: row.get("event_id"),
        event_type: row.get("event_type"),
        payload: row.get("payload_json"),
        url: row.get("url"),
        signing_secret: row.get("signing_secret"),
    }))
}

async fn handle_delivery_failure(
    state: &AppState,
    delivery_id: Uuid,
    current_attempt: i32,
    last_http_status: Option<i32>,
    last_error: String,
) -> Result<(), String> {
    let next_attempt = current_attempt + 1;

    if next_attempt >= MAX_ATTEMPTS {
        warn!(
            delivery_id = %delivery_id,
            attempt_number = next_attempt,
            "webhook delivery exhausted retry budget"
        );
        sqlx::query(
            "UPDATE webhook_deliveries
             SET status = 'exhausted',
                 attempt_number = $1,
                 last_http_status = $2,
                 last_error = $3,
                 next_attempt_at = NULL,
                 updated_at = now()
             WHERE id = $4",
        )
        .bind(next_attempt)
        .bind(last_http_status)
        .bind(last_error)
        .bind(delivery_id)
        .execute(&state.db)
        .await
        .map_err(|_| "failed to mark webhook delivery as exhausted".to_string())?;

        return Ok(());
    }

    let backoff_seconds = retry_backoff_seconds(next_attempt);
    info!(
        delivery_id = %delivery_id,
        attempt_number = next_attempt,
        retry_in_seconds = backoff_seconds,
        "webhook delivery scheduled for retry"
    );

    sqlx::query(
        "UPDATE webhook_deliveries
         SET status = 'pending',
             attempt_number = $1,
             last_http_status = $2,
             last_error = $3,
             next_attempt_at = now() + ($4 * interval '1 second'),
             updated_at = now()
         WHERE id = $5",
    )
    .bind(next_attempt)
    .bind(last_http_status)
    .bind(last_error)
    .bind(backoff_seconds)
    .bind(delivery_id)
    .execute(&state.db)
    .await
    .map_err(|_| "failed to schedule webhook retry".to_string())?;

    Ok(())
}

fn retry_backoff_seconds(next_attempt: i32) -> i32 {
    match next_attempt {
        1 => 5,
        2 => 30,
        3 => 120,
        4 => 600,
        _ => 1800,
    }
}

fn sign_payload(
    secret: &str,
    timestamp: &str,
    event_id: &str,
    payload: &str,
) -> Result<String, ()> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| ())?;
    let signed_data = format!("{timestamp}.{event_id}.{payload}");
    mac.update(signed_data.as_bytes());
    let bytes = mac.finalize().into_bytes();
    Ok(hex::encode(bytes))
}
