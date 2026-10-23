use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
    pub bind_addr: String,
    pub media_root: String,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://neko.db?mode=rwc".to_string()),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "change-me".to_string()),
            jwt_expiry_hours: env::var("JWT_EXPIRY_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(168),
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string()),
            media_root: env::var("MEDIA_ROOT").unwrap_or_else(|_| "./media".to_string()),
        }
    }
}
