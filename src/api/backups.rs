//! Backup endpoints.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header;
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::servers::types::ServerStatus;
use crate::AppState;

pub async fn list(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let backups = s.db.list_backups(&id).await?;
    Ok(Json(serde_json::json!({"backups": backups})))
}

#[derive(Deserialize)]
pub struct CreateReq {
    #[serde(default)]
    pub note: String,
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<CreateReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let rec =
        s.db.get_server(&id)
            .await?
            .ok_or_else(|| ApiError::not_found("server not found"))?;
    // If running, ask the server to save first.
    let _ = s.manager.send_command(&id, "save-all flush").await;
    let note = crate::backups::clean_note(&req.note);
    let row = crate::backups::create(
        &s.db,
        &id,
        &PathBuf::from(&rec.dir),
        &s.config.backups_dir(),
        &note,
    )
    .await?;
    s.db.audit(&user.username, "backup_create", &id).await.ok();
    Ok(Json(serde_json::json!({"backup": row})))
}

pub async fn restore(
    State(s): State<Arc<AppState>>,
    user: AuthUser,
    Path((id, bid)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let rt = s
        .manager
        .runtime(&id)
        .await
        .map_err(|_| ApiError::not_found("server not found"))?;
    if rt.status().await != ServerStatus::Stopped {
        return Err(ApiError::conflict("stop the server before restoring"));
    }
    let b =
        s.db.get_backup(&bid)
            .await?
            .filter(|b| b.server_id == id)
            .ok_or_else(|| ApiError::not_found("backup not found"))?;
    let rec = rt.record.read().await.clone();
    crate::backups::restore(std::path::Path::new(&b.path), &PathBuf::from(&rec.dir)).await?;
    s.db.audit(&user.username, "backup_restore", &format!("{id}:{bid}"))
        .await
        .ok();
    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn download(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path((id, bid)): Path<(String, String)>,
) -> ApiResult<axum::response::Response> {
    let b =
        s.db.get_backup(&bid)
            .await?
            .filter(|b| b.server_id == id)
            .ok_or_else(|| ApiError::not_found("backup not found"))?;
    let file = tokio::fs::File::open(&b.path)
        .await
        .map_err(|_| ApiError::not_found("backup file missing"))?;
    let name = crate::backups::download_name(std::path::Path::new(&b.path));
    let res = axum::response::Response::builder()
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{name}\""),
        )
        .body(Body::from_stream(tokio_util::io::ReaderStream::new(file)))
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(res)
}

pub async fn remove(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path((id, bid)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let b =
        s.db.get_backup(&bid)
            .await?
            .filter(|b| b.server_id == id)
            .ok_or_else(|| ApiError::not_found("backup not found"))?;
    crate::backups::delete(&s.db, &b.id).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}
