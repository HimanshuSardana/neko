use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{auth::AuthUser, models::Progress, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/progress", get(list).put(upsert))
}

type ApiErr = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiErr {
    (status, Json(serde_json::json!({ "error": msg.into() })))
}

#[derive(Debug, Deserialize, Default)]
struct ListQuery {
    media_kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpsertProgress {
    media_kind: String,
    episode_id: Option<String>,
    chapter_id: Option<String>,
    book_id: Option<String>,
    position_sec: Option<i32>,
    page: Option<i32>,
    completed: Option<bool>,
}

async fn list(
    user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<Progress>>, ApiErr> {
    if let Some(k) = &q.media_kind {
        if !["anime", "manga", "book"].contains(&k.as_str()) {
            return Err(err(StatusCode::BAD_REQUEST, "media_kind must be anime|manga|book"));
        }
    }
    let rows: Vec<Progress> = sqlx::query_as(
        "SELECT * FROM progress WHERE user_id = ? AND (? IS NULL OR media_kind = ?)
         ORDER BY updated_at DESC LIMIT 500",
    )
    .bind(&user.id)
    .bind(&q.media_kind)
    .bind(&q.media_kind)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    Ok(Json(rows))
}

async fn upsert(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<UpsertProgress>,
) -> Result<Json<Progress>, ApiErr> {
    let (table, col, id) = match body.media_kind.as_str() {
        "anime" => ("episodes", "episode_id", body.episode_id.clone()),
        "manga" => ("chapters", "chapter_id", body.chapter_id.clone()),
        "book" => ("books", "book_id", body.book_id.clone()),
        _ => return Err(err(StatusCode::BAD_REQUEST, "media_kind must be anime|manga|book")),
    };
    let Some(media_id) = id else {
        return Err(err(StatusCode::BAD_REQUEST, "matching media id required for media_kind"));
    };
    // Exactly one id must be set.
    let set = [body.episode_id.is_some(), body.chapter_id.is_some(), body.book_id.is_some()]
        .into_iter()
        .filter(|b| *b)
        .count();
    if set != 1 {
        return Err(err(StatusCode::BAD_REQUEST, "set exactly one of episode_id/chapter_id/book_id"));
    }
    let position_sec = body.position_sec.unwrap_or(0);
    let page = body.page.unwrap_or(0);
    if position_sec < 0 || page < 0 {
        return Err(err(StatusCode::BAD_REQUEST, "position_sec/page must be >= 0"));
    }
    let completed = body.completed.unwrap_or(false) as i32;

    // Media must exist.
    let exists: Option<(String,)> = sqlx::query_as(&format!("SELECT id FROM {table} WHERE id = ?"))
        .bind(&media_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    if exists.is_none() {
        return Err(err(StatusCode::NOT_FOUND, "media not found"));
    }

    let updated: usize = sqlx::query(&format!(
        "UPDATE progress SET position_sec = ?, page = ?, completed = ?,
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE user_id = ? AND {col} = ?"
    ))
    .bind(position_sec)
    .bind(page)
    .bind(completed)
    .bind(&user.id)
    .bind(&media_id)
    .execute(&state.pool)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?
    .rows_affected() as usize;

    if updated == 0 {
        let (e, c, b) = match body.media_kind.as_str() {
            "anime" => (Some(media_id.clone()), None, None),
            "manga" => (None, Some(media_id.clone()), None),
            _ => (None, None, Some(media_id.clone())),
        };
        sqlx::query(
            "INSERT INTO progress (user_id, media_kind, episode_id, chapter_id, book_id, position_sec, page, completed)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&user.id)
        .bind(&body.media_kind)
        .bind(e)
        .bind(c)
        .bind(b)
        .bind(position_sec)
        .bind(page)
        .bind(completed)
        .execute(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    }

    let row: Progress = sqlx::query_as(&format!("SELECT * FROM progress WHERE user_id = ? AND {col} = ?"))
        .bind(&user.id)
        .bind(&media_id)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    Ok(Json(row))
}
