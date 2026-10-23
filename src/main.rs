mod config;

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "neko" }))
}

fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/health", get(health))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "neko=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = Config::from_env();
    tracing::info!("neko starting, media_root={}", cfg.media_root);

    let app = router();
    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr)
        .await
        .expect("bind addr");
    tracing::info!("listening on {}", cfg.bind_addr);
    axum::serve(listener, app).await.expect("serve");
}
