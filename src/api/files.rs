//! File manager endpoints for a server's directory.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header;
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::files as fops;
use crate::AppState;

async fn root(s: &AppState, id: &str) -> ApiResult<PathBuf> {
    let rec = s
        .db
        .get_server(id)
        .await?
        .ok_or_else(|| ApiError::not_found("server not found"))?;
    Ok(PathBuf::from(rec.dir))
}

#[derive(Deserialize)]
pub struct PathQuery {
    #[serde(default)]
    pub path: String,
}

pub async fn list(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let root = root(&s, &id).await?;
    let entries = fops::list(&root, &q.path).await?;
    Ok(Json(serde_json::json!({"entries": entries, "path": q.path})))
}

pub async fn read(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let root = root(&s, &id).await?;
    let content = fops::read_text(&root, &q.path).await?;
    Ok(Json(serde_json::json!({"content": content, "path": q.path})))
}

#[derive(Deserialize)]
pub struct WriteReq {
    pub path: String,
    pub content: String,
}

pub async fn write(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<WriteReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let root = root(&s, &id).await?;
    fops::write_text(&root, &req.path, &req.content).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// Multipart upload: fields `path` (dir) + `file`.
pub async fn upload(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> ApiResult<Json<serde_json::Value>> {
    let root = root(&s, &id).await?;
    let mut dir = String::new();
    let mut saved = Vec::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("path") => dir = field.text().await.unwrap_or_default(),
            Some("file") => {
                let name = field
                    .file_name()
                    .map(crate::modrinth::sanitize_filename)
                    .unwrap_or_else(|| "upload.bin".into());
                let bytes = field.bytes().await.map_err(|e| ApiError::bad_request(e.to_string()))?;
                let rel = if dir.is_empty() { name.clone() } else { format!("{}/{}", dir.trim_end_matches('/'), name) };
                fops::write_bytes(&root, &rel, &bytes).await?;
                saved.push(rel);
            }
            _ => {}
        }
    }
    if saved.is_empty() {
        return Err(ApiError::bad_request("no file uploaded"));
    }
    Ok(Json(serde_json::json!({"ok": true, "files": saved})))
}

/// Download a file (octet-stream) — zips aren't created here; backups cover
/// directory export.
pub async fn download(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
) -> ApiResult<axum::response::Response> {
    let root = root(&s, &id).await?;
    let p = fops::safe_join(&root, &q.path)?;
    let md = tokio::fs::metadata(&p)
        .await
        .map_err(|_| ApiError::not_found("not found"))?;
    if md.is_dir() {
        return Err(ApiError::bad_request("directories can't be downloaded — use a backup"));
    }
    let file = tokio::fs::File::open(&p).await?;
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let res = axum::response::Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{name}\""),
        )
        .body(Body::from_stream(tokio_util::io::ReaderStream::new(file)))
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(res)
}

#[derive(Deserialize)]
pub struct MkdirReq {
    pub path: String,
}

pub async fn mkdir(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<MkdirReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let root = root(&s, &id).await?;
    if req.path.trim().is_empty() {
        return Err(ApiError::bad_request("path required"));
    }
    fops::mkdir(&root, &req.path).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct RenameReq {
    pub from: String,
    pub to: String,
}

pub async fn rename(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<RenameReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let root = root(&s, &id).await?;
    fops::rename(&root, &req.from, &req.to).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct DeleteReq {
    pub path: String,
}

pub async fn delete_file(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<DeleteReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let root = root(&s, &id).await?;
    fops::remove(&root, &req.path).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}
