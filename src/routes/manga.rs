use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    auth::AuthUser,
    models::{Chapter, Manga},
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/manga", get(list))
        .route("/manga/:id", get(get_one))
        .route("/manga/:id/chapters", get(chapters))
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
) -> Result<Json<Vec<Manga>>, ApiErr> {
    let rows: Vec<Manga> = sqlx::query_as(
        "SELECT * FROM manga
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
) -> Result<Json<Manga>, ApiErr> {
    let m: Option<Manga> = sqlx::query_as("SELECT * FROM manga WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    m.map(Json).ok_or_else(|| err(StatusCode::NOT_FOUND, "manga not found"))
}

async fn chapters(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Chapter>>, ApiErr> {
    let rows: Vec<Chapter> = sqlx::query_as(
        "SELECT * FROM chapters WHERE manga_id = ? ORDER BY chapter_number",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    Ok(Json(rows))
}
