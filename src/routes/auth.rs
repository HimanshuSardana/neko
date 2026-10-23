use axum::{extract::State, http::StatusCode, routing::{get, post}, Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{create_token, hash_password, verify_password, AuthUser},
    models::User,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/me", get(me))
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct UserPublic {
    id: String,
    username: String,
    email: String,
    role: String,
    created_at: DateTime<Utc>,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            role: u.role,
            created_at: u.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    token: String,
    user: UserPublic,
}

type ApiErr = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiErr {
    (status, Json(serde_json::json!({ "error": msg.into() })))
}

fn validate_creds(username: &str, password: &str, email: Option<&str>) -> Result<(), ApiErr> {
    let u = username.trim();
    if u.len() < 3 || u.len() > 32 {
        return Err(err(StatusCode::BAD_REQUEST, "username must be 3-32 chars"));
    }
    if password.len() < 8 {
        return Err(err(StatusCode::BAD_REQUEST, "password must be >= 8 chars"));
    }
    if let Some(e) = email {
        if !e.contains('@') || e.len() < 5 {
            return Err(err(StatusCode::BAD_REQUEST, "invalid email"));
        }
    }
    Ok(())
}

fn issue_token(state: &AppState, user: &User) -> Result<String, ApiErr> {
    create_token(
        &state.config.jwt_secret,
        &user.id,
        &user.username,
        &user.role,
        state.config.jwt_expiry_hours,
    )
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("token: {e}")))
}

/// First registered user becomes admin (standard self-hosted bootstrap).
async fn register(State(state): State<AppState>, Json(body): Json<RegisterRequest>) -> Result<Json<AuthResponse>, ApiErr> {
    validate_creds(&body.username, &body.password, Some(&body.email))?;

    let exists: Option<User> = sqlx::query_as("SELECT * FROM users WHERE username = ? OR email = ?")
        .bind(body.username.trim())
        .bind(body.email.trim())
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    if exists.is_some() {
        return Err(err(StatusCode::CONFLICT, "username or email already taken"));
    }

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    let role = if count.0 == 0 { "admin" } else { "user" };

    let hash = hash_password(&body.password)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("hash: {e}")))?;
    let id = uuid::Uuid::new_v4().to_string();

    let user: User = sqlx::query_as(
        "INSERT INTO users (id, username, email, password_hash, role) VALUES (?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(&id)
    .bind(body.username.trim())
    .bind(body.email.trim())
    .bind(&hash)
    .bind(role)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    let token = issue_token(&state, &user)?;
    Ok(Json(AuthResponse { token, user: user.into() }))
}

async fn login(State(state): State<AppState>, Json(body): Json<LoginRequest>) -> Result<Json<AuthResponse>, ApiErr> {
    if body.username.trim().is_empty() || body.password.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "username and password required"));
    }
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE username = ?")
        .bind(body.username.trim())
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    let Some(user) = user else {
        return Err(err(StatusCode::UNAUTHORIZED, "invalid credentials"));
    };
    if !verify_password(&user.password_hash, &body.password) {
        return Err(err(StatusCode::UNAUTHORIZED, "invalid credentials"));
    }
    let token = issue_token(&state, &user)?;
    Ok(Json(AuthResponse { token, user: user.into() }))
}

async fn me(user: AuthUser, State(state): State<AppState>) -> Result<Json<UserPublic>, ApiErr> {
    let found: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(&user.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    match found {
        Some(u) => Ok(Json(u.into())),
        None => Err(err(StatusCode::UNAUTHORIZED, "user no longer exists")),
    }
}
