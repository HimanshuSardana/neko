use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub role: String,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
    use rand::rngs::OsRng;
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash: {e}"))?
        .to_string())
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    use argon2::{PasswordHash, PasswordVerifier};
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    argon2::Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn create_token(secret: &str, user_id: &str, username: &str, role: &str, hours: i64) -> anyhow::Result<String> {
    let exp = (Utc::now() + Duration::hours(hours)).timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
        exp,
    };
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, axum::Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let err = |msg: &str| {
            (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({ "error": msg })),
            )
        };
        let hdr = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| err("missing bearer token"))?;
        let token = hdr.strip_prefix("Bearer ").ok_or_else(|| err("missing bearer token"))?;
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| err("invalid or expired token"))?;
        Ok(AuthUser {
            id: data.claims.sub,
            username: data.claims.username,
            role: data.claims.role,
        })
    }
}
