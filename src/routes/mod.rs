pub mod auth;
pub mod libraries;
pub mod anime;
pub mod manga;
pub mod books;

use crate::AppState;
use axum::Router;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .merge(libraries::router())
        .merge(anime::router())
        .merge(manga::router())
        .merge(books::router())
}
