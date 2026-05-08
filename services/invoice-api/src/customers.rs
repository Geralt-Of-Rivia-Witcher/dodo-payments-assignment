use axum::{
    extract::{Extension, Path, State},
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
pub struct CreateCustomerRequest {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct CustomerResponse {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_customer(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthBusiness>,
    Json(req): Json<CreateCustomerRequest>,
) -> Result<Json<CustomerResponse>, ApiError> {
    if req.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "validation_error",
            "name is required",
        ));
    }

    if req.email.trim().is_empty() {
        return Err(ApiError::bad_request(
            "validation_error",
            "email is required",
        ));
    }

    let id = Uuid::new_v4();

    let row = sqlx::query(
        "INSERT INTO customers (id, business_id, name, email) VALUES ($1, $2, $3, $4) RETURNING id, name, email, created_at",
    )
    .bind(id)
    .bind(auth.business_id)
    .bind(req.name.trim())
    .bind(req.email.trim())
    .fetch_one(&state.db)
    .await
    .map_err(map_customer_write_error)?;

    Ok(Json(CustomerResponse {
        id: row.get("id"),
        name: row.get("name"),
        email: row.get("email"),
        created_at: row.get("created_at"),
    }))
}

pub async fn get_customer(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthBusiness>,
    Path(customer_id): Path<Uuid>,
) -> Result<Json<CustomerResponse>, ApiError> {
    let row = sqlx::query(
        "SELECT id, name, email, created_at FROM customers WHERE id = $1 AND business_id = $2",
    )
    .bind(customer_id)
    .bind(auth.business_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| ApiError::internal("failed to fetch customer"))?;

    let Some(row) = row else {
        return Err(ApiError::not_found("customer not found"));
    };

    Ok(Json(CustomerResponse {
        id: row.get("id"),
        name: row.get("name"),
        email: row.get("email"),
        created_at: row.get("created_at"),
    }))
}

pub async fn list_customers(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthBusiness>,
) -> Result<Json<Vec<CustomerResponse>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, name, email, created_at FROM customers WHERE business_id = $1 ORDER BY created_at DESC",
    )
    .bind(auth.business_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| ApiError::internal("failed to list customers"))?;

    let customers = rows
        .into_iter()
        .map(|row| CustomerResponse {
            id: row.get("id"),
            name: row.get("name"),
            email: row.get("email"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(Json(customers))
}

fn map_customer_write_error(err: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(db_err) = &err {
        if db_err.code().as_deref() == Some("23505") {
            return ApiError::conflict(
                "duplicate_customer",
                "email already exists for this business",
            );
        }
    }

    ApiError::internal("failed to create customer")
}
