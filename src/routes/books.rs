use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{auth::AuthUser, models::Book, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/books", get(list))
        .route("/books/:id", get(get_one))
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
) -> Result<Json<Vec<Book>>, ApiErr> {
    let rows: Vec<Book> = sqlx::query_as(
        "SELECT * FROM books
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
) -> Result<Json<Book>, ApiErr> {
    let b: Option<Book> = sqlx::query_as("SELECT * FROM books WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    b.map(Json).ok_or_else(|| err(StatusCode::NOT_FOUND, "book not found"))
}
