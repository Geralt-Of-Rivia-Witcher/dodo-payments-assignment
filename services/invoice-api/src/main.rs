mod auth;
mod customers;
mod error;
mod invoices;
mod invoice_state;
mod payments;

use auth::{require_api_key, AppState};
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use customers::{create_customer, get_customer, list_customers};
use invoices::{create_invoice, get_invoice, list_invoices};
use payments::pay_invoice;
use sqlx::postgres::PgPoolOptions;
use std::env;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("connect postgres");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("run migrations");

    let state = AppState { db: pool };

    let public_routes = Router::new().route("/health", get(|| async { "ok" }));

    let protected_routes = Router::new()
        .route("/customers", post(create_customer).get(list_customers))
        .route("/customers/:id", get(get_customer))
        .route("/invoices", post(create_invoice).get(list_invoices))
        .route("/invoices/:id", get(get_invoice))
        .route("/invoices/:id/pay", post(pay_invoice));

    let app = public_routes
        .merge(protected_routes.route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_key,
        )))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("bind invoice-api");

    info!("invoice-api listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.expect("serve invoice-api");
}
