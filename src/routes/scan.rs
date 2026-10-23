use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use walkdir::WalkDir;

use crate::{auth::AuthUser, models::{Library, ScanEvent}, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/scan/:library_id", post(scan_library))
}

type ApiErr = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiErr {
    (status, Json(serde_json::json!({ "error": msg.into() })))
}

const VIDEO_EXTS: &[&str] = &["mkv", "mp4", "avi", "webm", "m4v", "mov"];
const BOOK_EXTS: &[&str] = &["epub", "pdf", "mobi", "azw3", "cbz", "cbr"];
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "avif"];

fn ext_of(name: &std::ffi::OsStr) -> String {
    std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn prettify(s: &str) -> String {
    s.replace(['.', '_'], " ").trim().to_string()
}

/// (season, episode) from a file stem. Handles S01E05, Ep 12, "Show - 12", bare numbers.
fn parse_episode(stem: &str) -> (i32, f64) {
    let lower = stem.to_lowercase();
    // S01E05 / s1e5
    if let Some(epos) = lower.find('e') {
        let (left, right) = (&stem[..epos], &stem[epos + 1..]);
        if let Some(spos) = left.to_lowercase().rfind('s') {
            if let (Ok(season), Ok(ep)) = (
                left[spos + 1..].trim().parse::<i32>(),
                right
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect::<String>()
                    .parse::<f64>(),
            ) {
                return (season.max(1), ep);
            }
        }
    }
    // "ep 12" / "episode 12"
    for marker in ["episode", "ep"] {
        if let Some(i) = lower.find(marker) {
            let num: String = stem[i + marker.len()..]
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(ep) = num.parse::<f64>() {
                return (1, ep);
            }
        }
    }
    // trailing number after separator
    for sep in [" - ", " _ ", " ", "-", "_"] {
        if let Some(i) = stem.rfind(sep) {
            if let Ok(ep) = stem[i + sep.len()..].trim().parse::<f64>() {
                return (1, ep);
            }
        }
    }
    // first integer anywhere
    let mut buf = String::new();
    for c in stem.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
        } else if !buf.is_empty() {
            break;
        }
    }
    (1, buf.parse::<f64>().unwrap_or(1.0))
}

fn first_float(s: &str) -> f64 {
    let mut buf = String::new();
    let mut dot = false;
    let mut started = false;
    for c in s.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
            started = true;
        } else if c == '.' && started && !dot {
            buf.push(c);
            dot = true;
        } else if started {
            break;
        }
    }
    buf.parse::<f64>().unwrap_or(1.0)
}

async fn find_or_create_series(state: &AppState, library_id: &str, title: &str) -> Result<String, sqlx::Error> {
    if let Some((id,)) = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM anime_series WHERE library_id = ? AND title = ?",
    )
    .bind(library_id)
    .bind(title)
    .fetch_optional(&state.pool)
    .await?
    {
        return Ok(id);
    }
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO anime_series (id, library_id, title) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(library_id)
        .bind(title)
        .execute(&state.pool)
        .await?;
    Ok(id)
}

async fn find_or_create_manga(state: &AppState, library_id: &str, title: &str) -> Result<String, sqlx::Error> {
    if let Some((id,)) = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM manga WHERE library_id = ? AND title = ?",
    )
    .bind(library_id)
    .bind(title)
    .fetch_optional(&state.pool)
    .await?
    {
        return Ok(id);
    }
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO manga (id, library_id, title) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(library_id)
        .bind(title)
        .execute(&state.pool)
        .await?;
    Ok(id)
}

async fn scan_anime(state: &AppState, lib: &Library) -> Result<i32, String> {
    let mut added = 0;
    for entry in WalkDir::new(&lib.root_path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !VIDEO_EXTS.contains(&ext_of(path.file_name().unwrap_or_default()).as_str()) {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
        let series_title = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(prettify)
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| lib.name.clone());
        let (season, ep) = parse_episode(stem);
        let series_id = find_or_create_series(state, &lib.id, &series_title)
            .await
            .map_err(|e| format!("db: {e}"))?;
        let fp = path.to_string_lossy().to_string();
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM episodes WHERE file_path = ?")
            .bind(&fp)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| format!("db: {e}"))?;
        if exists.is_none() {
            let id = uuid::Uuid::new_v4().to_string();
            let title = format!("Episode {ep}");
            sqlx::query(
                "INSERT INTO episodes (id, series_id, season_number, episode_number, title, file_path)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(&series_id)
            .bind(season)
            .bind(ep)
            .bind(&title)
            .bind(&fp)
            .execute(&state.pool)
            .await
            .map_err(|e| format!("db: {e}"))?;
            added += 1;
        }
    }
    Ok(added)
}

async fn scan_manga(state: &AppState, lib: &Library) -> Result<i32, String> {
    let mut added = 0;
    let root = std::path::Path::new(&lib.root_path);
    for entry in WalkDir::new(&lib.root_path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_dir() {
            continue;
        }
        let dir = entry.path();
        if dir == root {
            continue;
        }
        let has_images = std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok()).any(|e| {
                    e.file_type().map(|t| t.is_file()).unwrap_or(false)
                        && IMAGE_EXTS.contains(&ext_of(&e.file_name()).as_str())
                })
            })
            .unwrap_or(false);
        if !has_images {
            continue;
        }
        let chapter_name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("1");
        // Convention: media/manga/<Title>/<chapter-dir>. A directory directly
        // under root holding images is treated as a single-chapter title.
        let (manga_title, chapter_no) = if dir.parent() == Some(root) {
            (prettify(chapter_name), 1.0)
        } else {
            let parent = dir
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
                .unwrap_or(&lib.name);
            (prettify(parent), first_float(chapter_name))
        };
        let manga_id = find_or_create_manga(state, &lib.id, &manga_title)
            .await
            .map_err(|e| format!("db: {e}"))?;
        let fp = dir.to_string_lossy().to_string();
        let exists: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM chapters WHERE manga_id = ? AND chapter_number = ?",
        )
        .bind(&manga_id)
        .bind(chapter_no)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| format!("db: {e}"))?;
        if exists.is_none() {
            let id = uuid::Uuid::new_v4().to_string();
            let page_count = std::fs::read_dir(dir).map(|rd| rd.filter_map(|e| e.ok()).filter(|e| {
                e.file_type().map(|t| t.is_file()).unwrap_or(false)
                    && IMAGE_EXTS.contains(&ext_of(&e.file_name()).as_str())
            }).count()).unwrap_or(0) as i32;
            sqlx::query(
                "INSERT INTO chapters (id, manga_id, chapter_number, title, file_path, page_count)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(&manga_id)
            .bind(chapter_no)
            .bind(format!("Chapter {chapter_no}"))
            .bind(&fp)
            .bind(page_count)
            .execute(&state.pool)
            .await
            .map_err(|e| format!("db: {e}"))?;
            added += 1;
        }
    }
    Ok(added)
}

async fn scan_books(state: &AppState, lib: &Library) -> Result<i32, String> {
    let mut added = 0;
    for entry in WalkDir::new(&lib.root_path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = ext_of(path.file_name().unwrap_or_default());
        if !BOOK_EXTS.contains(&ext.as_str()) {
            continue;
        }
        let fp = path.to_string_lossy().to_string();
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM books WHERE file_path = ?")
            .bind(&fp)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| format!("db: {e}"))?;
        if exists.is_none() {
            let id = uuid::Uuid::new_v4().to_string();
            let title = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(prettify)
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "Unknown".to_string());
            sqlx::query("INSERT INTO books (id, library_id, title, file_path, format) VALUES (?, ?, ?, ?, ?)")
                .bind(&id)
                .bind(&lib.id)
                .bind(&title)
                .bind(&fp)
                .bind(&ext)
                .execute(&state.pool)
                .await
                .map_err(|e| format!("db: {e}"))?;
            added += 1;
        }
    }
    Ok(added)
}

async fn scan_library(
    user: AuthUser,
    State(state): State<AppState>,
    Path(library_id): Path<String>,
) -> Result<Json<ScanEvent>, ApiErr> {
    if !user.is_admin() {
        return Err(err(StatusCode::FORBIDDEN, "admin only"));
    }
    let lib: Option<Library> = sqlx::query_as("SELECT * FROM libraries WHERE id = ?")
        .bind(&library_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    let Some(lib) = lib else {
        return Err(err(StatusCode::NOT_FOUND, "library not found"));
    };

    let event_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO scan_events (id, library_id, status) VALUES (?, ?, 'running')")
        .bind(&event_id)
        .bind(&lib.id)
        .execute(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;

    let result = match lib.kind.as_str() {
        "anime" => scan_anime(&state, &lib).await,
        "manga" => scan_manga(&state, &lib).await,
        "book" => scan_books(&state, &lib).await,
        other => Err(format!("unknown library kind: {other}")),
    };

    match result {
        Ok(added) => {
            sqlx::query(
                "UPDATE scan_events SET status = 'done', files_added = ?,
                 finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
            )
            .bind(added)
            .bind(&event_id)
            .execute(&state.pool)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
            let event: ScanEvent = sqlx::query_as("SELECT * FROM scan_events WHERE id = ?")
                .bind(&event_id)
                .fetch_one(&state.pool)
                .await
                .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
            Ok(Json(event))
        }
        Err(msg) => {
            sqlx::query(
                "UPDATE scan_events SET status = 'error', message = ?,
                 finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
            )
            .bind(&msg)
            .bind(&event_id)
            .execute(&state.pool)
            .await
            .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
            Err(err(StatusCode::INTERNAL_SERVER_ERROR, msg))
        }
    }
}
