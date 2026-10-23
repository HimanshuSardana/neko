mod config;
mod db;
mod models;

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "neko" }))
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/health", get(health))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "neko=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = Arc::new(Config::from_env());
    let pool = db::connect(&cfg.database_url).await?;
    db::migrate(&pool).await?;
    tracing::info!("database ready ({})", cfg.database_url);

    // Ensure the media tree exists so a fresh self-hosted install just works.
    for dir in ["anime", "manga", "books"] {
        let p = std::path::Path::new(&cfg.media_root).join(dir);
        tokio::fs::create_dir_all(&p).await?;
    }
    tracing::info!("media root ready at {}", cfg.media_root);

    let state = AppState {
        pool,
        config: cfg.clone(),
    };
    let app = router(state);

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!("listening on {}", cfg.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
