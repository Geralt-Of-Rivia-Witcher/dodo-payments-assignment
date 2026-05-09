use invoice_api::{auth::AppState, build_app, webhook_worker::spawn_webhook_worker};
use sqlx::postgres::PgPoolOptions;
use std::{env, time::Duration};
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let psp_base_url = env::var("PSP_BASE_URL").expect("PSP_BASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("connect postgres");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("run migrations");

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .expect("build http client");

    let state = AppState {
        db: pool,
        psp_base_url,
        http_client,
    };

    spawn_webhook_worker(state.clone());
    let app = build_app(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("bind invoice-api");

    info!("invoice-api listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.expect("serve invoice-api");
}
