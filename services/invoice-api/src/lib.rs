pub mod auth;
pub mod customers;
pub mod error;
pub mod invoice_state;
pub mod invoices;
pub mod payments;
pub mod webhook_outbox;
pub mod webhook_worker;
pub mod webhooks;

use auth::{require_api_key, AppState};
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use customers::{create_customer, get_customer, list_customers};
use invoices::{create_invoice, get_invoice, list_invoices};
use payments::pay_invoice;
use webhooks::{create_webhook_endpoint, list_webhook_endpoints};

pub fn build_app(state: AppState) -> Router {
    let public_routes = Router::new().route("/health", get(|| async { "ok" }));

    let protected_routes = Router::new()
        .route("/customers", post(create_customer).get(list_customers))
        .route("/customers/:id", get(get_customer))
        .route("/invoices", post(create_invoice).get(list_invoices))
        .route("/invoices/:id", get(get_invoice))
        .route("/invoices/:id/pay", post(pay_invoice))
        .route(
            "/webhook-endpoints",
            post(create_webhook_endpoint).get(list_webhook_endpoints),
        );

    public_routes
        .merge(protected_routes.route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_key,
        )))
        .with_state(state)
}
