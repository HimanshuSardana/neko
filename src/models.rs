use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Library {
    pub id: String,
    pub name: String,
    /// anime | manga | book
    pub kind: String,
    pub root_path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AnimeSeries {
    pub id: String,
    pub library_id: String,
    pub title: String,
    pub romaji_title: Option<String>,
    pub description: Option<String>,
    pub cover_path: Option<String>,
    pub status: String,
    pub year: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Episode {
    pub id: String,
    pub series_id: String,
    pub season_number: i32,
    pub episode_number: f64,
    pub title: Option<String>,
    pub file_path: String,
    pub duration_sec: Option<i32>,
    pub size_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Manga {
    pub id: String,
    pub library_id: String,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub cover_path: Option<String>,
    pub status: String,
    pub year: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Chapter {
    pub id: String,
    pub manga_id: String,
    pub chapter_number: f64,
    pub title: Option<String>,
    pub file_path: String,
    pub page_count: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Book {
    pub id: String,
    pub library_id: String,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub cover_path: Option<String>,
    pub file_path: String,
    pub format: Option<String>,
    pub page_count: Option<i32>,
    pub year: Option<i32>,
    pub size_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Progress {
    pub id: i64,
    pub user_id: String,
    /// anime | manga | book
    pub media_kind: String,
    pub episode_id: Option<String>,
    pub chapter_id: Option<String>,
    pub book_id: Option<String>,
    pub position_sec: i32,
    pub page: i32,
    pub completed: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScanEvent {
    pub id: String,
    pub library_id: String,
    pub status: String,
    pub files_added: i32,
    pub message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}
