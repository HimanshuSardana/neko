pub mod auth;

use crate::AppState;
use axum::Router;

pub fn api_router() -> Router<AppState> {
    Router::new().nest("/auth", auth::router())
}
