use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{auth::AuthUser, models::Library, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/libraries", get(list).post(create))
        .route("/libraries/:id", get(get_one).delete(delete_one))
}

type ApiErr = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiErr {
    (status, Json(serde_json::json!({ "error": msg.into() })))
}

#[derive(Debug, Deserialize)]
struct CreateLibrary {
    name: String,
    kind: String,
    root_path: String,
}

async fn list(_user: AuthUser, State(state): State<AppState>) -> Result<Json<Vec<Library>>, ApiErr> {
    let libs: Vec<Library> = sqlx::query_as("SELECT * FROM libraries ORDER BY name")
        .fetch_all(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    Ok(Json(libs))
}

async fn get_one(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Library>, ApiErr> {
    let lib: Option<Library> = sqlx::query_as("SELECT * FROM libraries WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    lib.map(Json).ok_or_else(|| err(StatusCode::NOT_FOUND, "library not found"))
}

/// Admin-only: register a library root and create the directory.
async fn create(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateLibrary>,
) -> Result<(StatusCode, Json<Library>), ApiErr> {
    if !user.is_admin() {
        return Err(err(StatusCode::FORBIDDEN, "admin only"));
    }
    if !["anime", "manga", "book"].contains(&body.kind.as_str()) {
        return Err(err(StatusCode::BAD_REQUEST, "kind must be anime|manga|book"));
    }
    if body.name.trim().is_empty() || body.root_path.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "name and root_path required"));
    }

    tokio::fs::create_dir_all(&body.root_path)
        .await
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("bad root_path: {e}")))?;

    let id = uuid::Uuid::new_v4().to_string();
    let lib: Library = sqlx::query_as(
        "INSERT INTO libraries (id, name, kind, root_path) VALUES (?, ?, ?, ?) RETURNING *",
    )
    .bind(&id)
    .bind(body.name.trim())
    .bind(&body.kind)
    .bind(body.root_path.trim())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| err(StatusCode::CONFLICT, format!("db: {e}")))?;
    Ok((StatusCode::CREATED, Json(lib)))
}

async fn delete_one(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiErr> {
    if !user.is_admin() {
        return Err(err(StatusCode::FORBIDDEN, "admin only"));
    }
    let r = sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    if r.rows_affected() == 0 {
        return Err(err(StatusCode::NOT_FOUND, "library not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}
