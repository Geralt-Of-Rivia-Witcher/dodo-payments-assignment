use axum::{routing::get, Router};
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let app = Router::new().route("/health", get(|| async { "ok" }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("bind invoice-api");

    info!("invoice-api listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.expect("serve invoice-api");
}
