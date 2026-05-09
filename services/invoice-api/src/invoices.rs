use axum::{
    extract::{Extension, Path, Query, State},
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
pub struct CreateInvoiceRequest {
    pub customer_id: Uuid,
    pub due_date: chrono::NaiveDate,
    pub line_items: Vec<CreateInvoiceLineItem>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvoiceLineItem {
    pub description: String,
    pub quantity: i32,
    pub unit_amount_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct ListInvoicesQuery {
    pub state: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InvoiceResponse {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub state: String,
    pub total_amount_cents: i64,
    pub due_date: chrono::NaiveDate,
    pub line_items: Vec<InvoiceLineItemResponse>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct InvoiceSummaryResponse {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub state: String,
    pub total_amount_cents: i64,
    pub due_date: chrono::NaiveDate,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct InvoiceLineItemResponse {
    pub description: String,
    pub quantity: i32,
    pub unit_amount_cents: i64,
}

pub async fn create_invoice(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthBusiness>,
    Json(req): Json<CreateInvoiceRequest>,
) -> Result<Json<InvoiceResponse>, ApiError> {
    if req.line_items.is_empty() {
        return Err(ApiError::bad_request(
            "validation_error",
            "line_items cannot be empty",
        ));
    }

    let mut total_amount_cents: i64 = 0;

    for item in &req.line_items {
        if item.description.trim().is_empty() {
            return Err(ApiError::bad_request(
                "validation_error",
                "line item description is required",
            ));
        }

        if item.quantity <= 0 {
            return Err(ApiError::bad_request(
                "validation_error",
                "line item quantity must be greater than 0",
            ));
        }

        if item.unit_amount_cents < 0 {
            return Err(ApiError::bad_request(
                "validation_error",
                "line item unit_amount_cents must be >= 0",
            ));
        }

        let line_total = item
            .unit_amount_cents
            .checked_mul(item.quantity as i64)
            .ok_or_else(|| ApiError::bad_request("validation_error", "line item total overflow"))?;

        total_amount_cents = total_amount_cents
            .checked_add(line_total)
            .ok_or_else(|| ApiError::bad_request("validation_error", "invoice total overflow"))?;
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::internal("failed to start transaction"))?;

    let customer_exists = sqlx::query("SELECT 1 FROM customers WHERE id = $1 AND business_id = $2")
        .bind(req.customer_id)
        .bind(auth.business_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ApiError::internal("failed to verify customer"))?;

    if customer_exists.is_none() {
        return Err(ApiError::not_found("customer not found"));
    }

    let invoice_id = Uuid::new_v4();

    let invoice_row = sqlx::query(
        "INSERT INTO invoices (id, business_id, customer_id, state, total_amount_cents, due_date) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, customer_id, state, total_amount_cents, due_date, created_at, updated_at",
    )
    .bind(invoice_id)
    .bind(auth.business_id)
    .bind(req.customer_id)
    .bind("open")
    .bind(total_amount_cents)
    .bind(req.due_date)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| ApiError::internal("failed to create invoice"))?;

    for item in &req.line_items {
        sqlx::query(
            "INSERT INTO invoice_line_items (id, invoice_id, description, quantity, unit_amount_cents) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(Uuid::new_v4())
        .bind(invoice_id)
        .bind(item.description.trim())
        .bind(item.quantity)
        .bind(item.unit_amount_cents)
        .execute(&mut *tx)
        .await
        .map_err(|_| ApiError::internal("failed to create invoice line item"))?;
    }

    tx.commit()
        .await
        .map_err(|_| ApiError::internal("failed to commit invoice transaction"))?;

    let response = InvoiceResponse {
        id: invoice_row.get("id"),
        customer_id: invoice_row.get("customer_id"),
        state: invoice_row.get("state"),
        total_amount_cents: invoice_row.get("total_amount_cents"),
        due_date: invoice_row.get("due_date"),
        line_items: req
            .line_items
            .into_iter()
            .map(|item| InvoiceLineItemResponse {
                description: item.description.trim().to_string(),
                quantity: item.quantity,
                unit_amount_cents: item.unit_amount_cents,
            })
            .collect(),
        created_at: invoice_row.get("created_at"),
        updated_at: invoice_row.get("updated_at"),
    };

    Ok(Json(response))
}

pub async fn get_invoice(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthBusiness>,
    Path(invoice_id): Path<Uuid>,
) -> Result<Json<InvoiceResponse>, ApiError> {
    let invoice_row = sqlx::query(
        "SELECT id, customer_id, state, total_amount_cents, due_date, created_at, updated_at
         FROM invoices
         WHERE id = $1 AND business_id = $2",
    )
    .bind(invoice_id)
    .bind(auth.business_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| ApiError::internal("failed to fetch invoice"))?;

    let Some(invoice_row) = invoice_row else {
        return Err(ApiError::not_found("invoice not found"));
    };

    let item_rows = sqlx::query(
        "SELECT description, quantity, unit_amount_cents
         FROM invoice_line_items
         WHERE invoice_id = $1
         ORDER BY id",
    )
    .bind(invoice_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| ApiError::internal("failed to fetch invoice line items"))?;

    let line_items = item_rows
        .into_iter()
        .map(|row| InvoiceLineItemResponse {
            description: row.get("description"),
            quantity: row.get("quantity"),
            unit_amount_cents: row.get("unit_amount_cents"),
        })
        .collect();

    Ok(Json(InvoiceResponse {
        id: invoice_row.get("id"),
        customer_id: invoice_row.get("customer_id"),
        state: invoice_row.get("state"),
        total_amount_cents: invoice_row.get("total_amount_cents"),
        due_date: invoice_row.get("due_date"),
        line_items,
        created_at: invoice_row.get("created_at"),
        updated_at: invoice_row.get("updated_at"),
    }))
}

pub async fn list_invoices(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthBusiness>,
    Query(query): Query<ListInvoicesQuery>,
) -> Result<Json<Vec<InvoiceSummaryResponse>>, ApiError> {
    if let Some(ref state_filter) = query.state {
        if !is_valid_invoice_state(state_filter) {
            return Err(ApiError::bad_request(
                "validation_error",
                "invalid invoice state filter",
            ));
        }
    }

    let rows = if let Some(state_filter) = query.state {
        sqlx::query(
            "SELECT id, customer_id, state, total_amount_cents, due_date, created_at, updated_at
             FROM invoices
             WHERE business_id = $1 AND state = $2
             ORDER BY created_at DESC",
        )
        .bind(auth.business_id)
        .bind(state_filter)
        .fetch_all(&state.db)
        .await
        .map_err(|_| ApiError::internal("failed to list invoices"))?
    } else {
        sqlx::query(
            "SELECT id, customer_id, state, total_amount_cents, due_date, created_at, updated_at
             FROM invoices
             WHERE business_id = $1
             ORDER BY created_at DESC",
        )
        .bind(auth.business_id)
        .fetch_all(&state.db)
        .await
        .map_err(|_| ApiError::internal("failed to list invoices"))?
    };

    let invoices = rows
        .into_iter()
        .map(|row| InvoiceSummaryResponse {
            id: row.get("id"),
            customer_id: row.get("customer_id"),
            state: row.get("state"),
            total_amount_cents: row.get("total_amount_cents"),
            due_date: row.get("due_date"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect();

    Ok(Json(invoices))
}

fn is_valid_invoice_state(state: &str) -> bool {
    matches!(state, "draft" | "open" | "paid" | "void" | "uncollectible")
}
