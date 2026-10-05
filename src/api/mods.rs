//! Modrinth search/install endpoints.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn search(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let rec = s
        .db
        .get_server(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("server not found"))?;
    let loader = crate::servers::types::ServerType::parse(&rec.server_type)
        .and_then(|t| t.modrinth_facets())
        .ok_or_else(|| ApiError::bad_request("this server type doesn't support mods/plugins"))?;
    let project_type = if rec.server_type == "paper" || rec.server_type == "purpur" {
        "plugin"
    } else {
        "mod"
    };
    let hits = crate::modrinth::search(
        &s.http,
        &q.q,
        loader,
        project_type,
        &rec.mc_version,
        q.limit.unwrap_or(20),
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(serde_json::json!({"hits": hits})))
}

#[derive(Deserialize)]
pub struct InstallReq {
    pub project: String, // slug or id
}

pub async fn install(
    State(s): State<Arc<AppState>>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<InstallReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let rec = s
        .db
        .get_server(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("server not found"))?;
    let dir_name = crate::modrinth::content_dir(&PathBuf::from(&rec.dir), &rec.server_type)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let loader = crate::servers::types::ServerType::parse(&rec.server_type)
        .and_then(|t| t.modrinth_facets())
        .unwrap_or("");
    let filename = crate::modrinth::install(
        &s.http,
        &req.project,
        loader,
        &rec.mc_version,
        &PathBuf::from(&rec.dir).join(dir_name),
    )
    .await
    .map_err(|e| ApiError::bad_request(e.to_string()))?;
    s.db.audit(&user.username, "mod_install", &format!("{id}:{}", req.project)).await.ok();
    Ok(Json(serde_json::json!({"ok": true, "file": filename})))
}

/// Currently installed mod/plugin jar files.
pub async fn installed(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let rec = s
        .db
        .get_server(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("server not found"))?;
    let dir_name = crate::servers::types::ServerType::parse(&rec.server_type)
        .map(|t| t.content_dir())
        .unwrap_or("mods");
    let dir = PathBuf::from(&rec.dir).join(dir_name);
    let mut files = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(".jar") {
                let size = e.metadata().await.map(|m| m.len()).unwrap_or(0);
                files.push(serde_json::json!({"name": name, "size": size}));
            }
        }
    }
    files.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(Json(serde_json::json!({"files": files, "dir": dir_name})))
}
