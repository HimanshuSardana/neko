use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    auth::AuthUser,
    models::{AnimeSeries, Episode},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/anime", get(list))
        .route("/anime/:id", get(get_one))
        .route("/anime/:id/episodes", get(episodes))
}

type ApiErr = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiErr {
    (status, Json(serde_json::json!({ "error": msg.into() })))
}

#[derive(Debug, Deserialize, Default)]
struct ListQuery {
    library_id: Option<String>,
    q: Option<String>,
}

async fn list(
    _user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<AnimeSeries>>, ApiErr> {
    let rows: Vec<AnimeSeries> = sqlx::query_as(
        "SELECT * FROM anime_series
         WHERE (? IS NULL OR library_id = ?)
           AND (? IS NULL OR title LIKE '%' || ? || '%')
         ORDER BY title LIMIT 200",
    )
    .bind(&q.library_id)
    .bind(&q.library_id)
    .bind(&q.q)
    .bind(&q.q)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    Ok(Json(rows))
}

async fn get_one(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AnimeSeries>, ApiErr> {
    let s: Option<AnimeSeries> = sqlx::query_as("SELECT * FROM anime_series WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    s.map(Json).ok_or_else(|| err(StatusCode::NOT_FOUND, "series not found"))
}

async fn episodes(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Episode>>, ApiErr> {
    let rows: Vec<Episode> = sqlx::query_as(
        "SELECT * FROM episodes WHERE series_id = ? ORDER BY season_number, episode_number",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    Ok(Json(rows))
}
