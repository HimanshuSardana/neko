use axum::{
    body::Body,
    extract::{Path, State},
    http::{
        header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE},
        HeaderMap, StatusCode,
    },
    routing::get,
    Json, Router,
};
use std::path::Path as FsPath;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::{auth::AuthUser, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stream/episode/:id", get(stream_episode))
        .route("/stream/chapter/:id/pages", get(chapter_pages))
        .route("/stream/chapter/:id/page/:n", get(chapter_page))
        .route("/stream/book/:id", get(stream_book))
}

type ApiErr = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiErr {
    (status, Json(serde_json::json!({ "error": msg.into() })))
}

fn content_type(path: &str) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string()
}

/// Parse `Range: bytes=start-end` (suffix ranges not supported in v1).
fn parse_range(headers: &HeaderMap, len: u64) -> Result<Option<(u64, u64)>, ApiErr> {
    let Some(v) = headers.get("range").and_then(|v| v.to_str().ok()) else {
        return Ok(None);
    };
    let v = v.strip_prefix("bytes=").ok_or_else(|| err(StatusCode::RANGE_NOT_SATISFIABLE, "bad range unit"))?;
    let (s, e) = v.split_once('-').ok_or_else(|| err(StatusCode::RANGE_NOT_SATISFIABLE, "bad range"))?;
    let start: u64 = s.parse().map_err(|_| err(StatusCode::RANGE_NOT_SATISFIABLE, "bad range start"))?;
    let end: u64 = if e.is_empty() {
        len.saturating_sub(1)
    } else {
        e.parse().map_err(|_| err(StatusCode::RANGE_NOT_SATISFIABLE, "bad range end"))?
    };
    if start >= len || end < start {
        return Err(err(StatusCode::RANGE_NOT_SATISFIABLE, "range out of bounds"));
    }
    Ok(Some((start, end.min(len - 1))))
}

async fn ranged_file(path: &str, headers: &HeaderMap) -> Result<(StatusCode, HeaderMap, Body), ApiErr> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "file missing on disk"))?;
    let len = meta.len();
    let ctype = content_type(path);

    let mut out = HeaderMap::new();
    out.insert(ACCEPT_RANGES, "bytes".parse().unwrap());
    out.insert(CONTENT_TYPE, ctype.parse().unwrap());

    match parse_range(headers, len)? {
        None => {
            let f = tokio::fs::File::open(path)
                .await
                .map_err(|_| err(StatusCode::NOT_FOUND, "file missing on disk"))?;
            out.insert(CONTENT_LENGTH, len.to_string().parse().unwrap());
            Ok((StatusCode::OK, out, Body::from_stream(ReaderStream::new(f))))
        }
        Some((start, end)) => {
            let mut f = tokio::fs::File::open(path)
                .await
                .map_err(|_| err(StatusCode::NOT_FOUND, "file missing on disk"))?;
            f.seek(std::io::SeekFrom::Start(start))
                .await
                .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("seek: {e}")))?;
            let n = end - start + 1;
            out.insert(CONTENT_LENGTH, n.to_string().parse().unwrap());
            out.insert(
                CONTENT_RANGE,
                format!("bytes {start}-{end}/{len}").parse().unwrap(),
            );
            Ok((
                StatusCode::PARTIAL_CONTENT,
                out,
                Body::from_stream(ReaderStream::new(f.take(n))),
            ))
        }
    }
}

async fn stream_episode(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<(StatusCode, HeaderMap, Body), ApiErr> {
    let _ = user;
    let row: Option<(String,)> = sqlx::query_as("SELECT file_path FROM episodes WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    let Some((path,)) = row else {
        return Err(err(StatusCode::NOT_FOUND, "episode not found"));
    };
    ranged_file(&path, &headers).await
}

async fn stream_book(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<(StatusCode, HeaderMap, Body), ApiErr> {
    let _ = user;
    let row: Option<(String,)> = sqlx::query_as("SELECT file_path FROM books WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    let Some((path,)) = row else {
        return Err(err(StatusCode::NOT_FOUND, "book not found"));
    };
    ranged_file(&path, &headers).await
}

fn is_image(p: &FsPath) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("webp") | Some("gif") | Some("avif")
    )
}

async fn chapter_dir(state: &AppState, id: &str) -> Result<String, ApiErr> {
    let row: Option<(String,)> = sqlx::query_as("SELECT file_path FROM chapters WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("db: {e}")))?;
    row.map(|r| r.0).ok_or_else(|| err(StatusCode::NOT_FOUND, "chapter not found"))
}

async fn sorted_images(dir: &str) -> Result<Vec<String>, ApiErr> {
    let mut entries = tokio::fs::read_dir(dir)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "chapter dir missing on disk"))?;
    let mut imgs = vec![];
    while let Some(e) = entries
        .next_entry()
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("read dir: {e}")))?
    {
        let p = e.path();
        if is_image(&p) {
            imgs.push(p.to_string_lossy().to_string());
        }
    }
    imgs.sort();
    Ok(imgs)
}

/// List page image names for a chapter directory.
async fn chapter_pages(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErr> {
    let _ = user;
    let dir = chapter_dir(&state, &id).await?;
    if !FsPath::new(&dir).is_dir() {
        return Err(err(StatusCode::UNSUPPORTED_MEDIA_TYPE, "chapter is not a directory of images (cbz unsupported in v1)"));
    }
    let imgs = sorted_images(&dir).await?;
    Ok(Json(serde_json::json!({ "chapter_id": id, "pages": imgs.len() })))
}

/// Serve page n (0-based) of a chapter directory.
async fn chapter_page(
    user: AuthUser,
    State(state): State<AppState>,
    Path((id, n)): Path<(String, usize)>,
) -> Result<(StatusCode, HeaderMap, Body), ApiErr> {
    let _ = user;
    let dir = chapter_dir(&state, &id).await?;
    if !FsPath::new(&dir).is_dir() {
        return Err(err(StatusCode::UNSUPPORTED_MEDIA_TYPE, "chapter is not a directory of images (cbz unsupported in v1)"));
    }
    let imgs = sorted_images(&dir).await?;
    let path = imgs.get(n).ok_or_else(|| err(StatusCode::NOT_FOUND, "page out of range"))?;
    let data = tokio::fs::read(path)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "page missing on disk"))?;
    let mut out = HeaderMap::new();
    out.insert(CONTENT_TYPE, content_type(path).parse().unwrap());
    out.insert(CONTENT_LENGTH, data.len().to_string().parse().unwrap());
    Ok((StatusCode::OK, out, Body::from(data)))
}
