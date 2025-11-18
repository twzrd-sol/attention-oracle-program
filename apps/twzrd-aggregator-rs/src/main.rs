use axum::{routing::{get, post}, Router};
use dotenvy::dotenv;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod db;
mod ingest;
mod merkle;
mod state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let metrics_handle = install_metrics();

    // Initialize DB pool if configured
    let pool = db::init_pool(std::env::var("DATABASE_URL").ok().as_deref()).await?;
    let app_state = state::AppState { pool, metrics: metrics_handle.clone() };

    let app = Router::new()
        .route("/health", get(api::health))
        .route("/metrics", get(move || async move { metrics_handle.render() }))
        .route("/ingest", post(ingest::ingest_handler))
        .route("/proof", get(api::not_implemented))
        .with_state(app_state);

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let addr: SocketAddr = format!("{}:{}", host, port).parse().expect("invalid HOST/PORT");
    info!("listening on {}", addr);
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn install_metrics() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("install metrics recorder")
}
