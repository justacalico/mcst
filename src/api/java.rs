//! Java runtime endpoints: detection, managed installs, requirement lookup.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

/// Detected + managed runtimes.
pub async fn list(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
) -> ApiResult<Json<serde_json::Value>> {
    let mut installs = crate::java::detect(&s.config.java_dir()).await;
    let managed = s.db.list_java().await?;
    for m in managed {
        if !installs.iter().any(|j| j.path == m.path) {
            installs.push(crate::java::JavaInstall {
                path: m.path,
                major: m.major as u32,
                version: String::new(),
                managed: m.managed != 0,
            });
        }
    }
    installs.sort_by_key(|j| j.major);
    Ok(Json(serde_json::json!({"installs": installs})))
}

#[derive(Deserialize)]
pub struct InstallReq {
    pub major: u32,
}

/// Download a Temurin JRE.
pub async fn install(
    State(s): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<InstallReq>,
) -> ApiResult<Json<serde_json::Value>> {
    if !(8..=99).contains(&req.major) {
        return Err(ApiError::bad_request("invalid java major version"));
    }
    let dest = s.config.java_dir().join(format!("jdk-{}", req.major));
    let path = crate::java::install(&s.http, req.major, &dest)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    s.db.insert_java(req.major as i64, &path, true).await?;
    s.db.audit(
        &user.username,
        "java_install",
        &format!("java {}", req.major),
    )
    .await
    .ok();
    Ok(Json(serde_json::json!({"ok": true, "path": path})))
}

/// Remove a managed runtime from the registry (and disk if managed).
pub async fn remove(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let rows = s.db.list_java().await?;
    if let Some(r) = rows.iter().find(|r| r.id == id) {
        if r.managed != 0 {
            // Remove the unpacked tree (parent of .../bin/java).
            let p = std::path::PathBuf::from(&r.path);
            if let Some(bin) = p.parent() {
                if let Some(jdk) = bin.parent() {
                    if jdk.starts_with(s.config.java_dir()) {
                        let _ = tokio::fs::remove_dir_all(jdk).await;
                    }
                }
            }
        }
        s.db.delete_java(&id).await?;
    }
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Deserialize)]
pub struct RequiredQuery {
    pub mc_version: String,
}

/// Java major required for a Minecraft version.
pub async fn required(_u: AuthUser, Query(q): Query<RequiredQuery>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"major": crate::java::required_major(&q.mc_version)}))
}
