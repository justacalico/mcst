//! Server CRUD, lifecycle control, properties, icon, console command.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Multipart, Path, State};
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::servers::props::Properties;
use crate::servers::types::{ServerRecord, ServerStatus};
use crate::servers::CreateServer;
use crate::AppState;

pub async fn list(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
) -> ApiResult<Json<serde_json::Value>> {
    let servers = s.manager.list_dtos().await?;
    Ok(Json(serde_json::json!({"servers": servers})))
}

pub async fn get(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let dto = s
        .manager
        .dto(&id)
        .await
        .map_err(|_| ApiError::not_found("server not found"))?;
    Ok(Json(serde_json::to_value(dto).unwrap_or_default()))
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<CreateServer>,
) -> ApiResult<Json<serde_json::Value>> {
    req.validate().map_err(ApiError::bad_request)?;
    // Port uniqueness among managed servers.
    for other in s.db.list_servers().await? {
        if other.port == req.port {
            return Err(ApiError::conflict(format!(
                "port {} is already used by '{}'",
                req.port, other.name
            )));
        }
    }
    let record = req.to_record(&s.config.servers_dir());
    let id = record.id.clone();
    s.db.insert_server(&record).await?;
    let rt = s.manager.runtime(&id).await?;
    rt.force_status(ServerStatus::Installing).await;
    s.db.audit(
        &user.username,
        "server_create",
        &format!("{} ({})", req.name, req.server_type),
    )
    .await
    .ok();

    // Install in the background; progress streams into the console log.
    let catalog = s.catalog.clone();
    let http = s.http.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::install::install_server(&http, &catalog, &rt).await {
            rt.push_log(format!("[mcst] install failed: {e}")).await;
        }
        rt.force_status(ServerStatus::Stopped).await;
    });

    let dto = s.manager.dto(&id).await?;
    Ok(Json(serde_json::to_value(dto).unwrap_or_default()))
}

#[derive(Deserialize, Default)]
pub struct PatchServer {
    pub name: Option<String>,
    pub port: Option<i64>,
    pub memory_mb: Option<i64>,
    pub min_memory_mb: Option<i64>,
    pub java_path: Option<String>,
    pub jvm_args: Option<String>,
    pub auto_start: Option<bool>,
    pub restart_on_crash: Option<bool>,
    pub shutdown_timeout_sec: Option<i64>,
    pub empty_stop_minutes: Option<i64>,
}

pub async fn update(
    State(s): State<Arc<AppState>>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<PatchServer>,
) -> ApiResult<Json<serde_json::Value>> {
    let rt = s
        .manager
        .runtime(&id)
        .await
        .map_err(|_| ApiError::not_found("server not found"))?;
    if rt.status().await.is_active() {
        return Err(ApiError::conflict("stop the server before editing it"));
    }
    let mut rec = rt.record.read().await.clone();
    if let Some(v) = patch.name {
        crate::servers::command::validate_server_name(&v).map_err(ApiError::bad_request)?;
        rec.name = v;
    }
    if let Some(v) = patch.port {
        if !(1024..=65535).contains(&v) {
            return Err(ApiError::bad_request("port must be 1024-65535"));
        }
        for other in s.db.list_servers().await? {
            if other.port == v && other.id != id {
                return Err(ApiError::conflict("port is already in use"));
            }
        }
        rec.port = v;
        // Keep server.properties in sync.
        let props_path = PathBuf::from(&rec.dir).join("server.properties");
        if let Ok(mut p) = Properties::load(&props_path).await {
            p.set("server-port", &v.to_string());
            let _ = p.save(&props_path).await;
        }
    }
    if let Some(v) = patch.memory_mb {
        if !(256..=262144).contains(&v) {
            return Err(ApiError::bad_request("memory_mb must be 256-262144"));
        }
        rec.memory_mb = v;
    }
    if let Some(v) = patch.min_memory_mb {
        rec.min_memory_mb = v.max(0);
    }
    if let Some(v) = patch.java_path {
        rec.java_path = if v.is_empty() { "java".into() } else { v };
    }
    if let Some(v) = patch.jvm_args {
        rec.jvm_args = v;
    }
    if let Some(v) = patch.auto_start {
        rec.auto_start = v as i64;
    }
    if let Some(v) = patch.restart_on_crash {
        rec.restart_on_crash = v as i64;
    }
    if let Some(v) = patch.shutdown_timeout_sec {
        rec.shutdown_timeout_sec = v.clamp(1, 3600);
    }
    if let Some(v) = patch.empty_stop_minutes {
        rec.empty_stop_minutes = v.clamp(0, 10080);
    }
    s.db.update_server(&rec).await?;
    *rt.record.write().await = rec;
    s.db.audit(&user.username, "server_update", &id).await.ok();
    let dto = s.manager.dto(&id).await?;
    Ok(Json(serde_json::to_value(dto).unwrap_or_default()))
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let rt = s
        .manager
        .runtime(&id)
        .await
        .map_err(|_| ApiError::not_found("server not found"))?;
    if rt.status().await.is_active() {
        return Err(ApiError::conflict("stop the server before deleting it"));
    }
    let rec = rt.record.read().await.clone();
    s.manager.evict(&id).await;
    s.db.delete_server(&id).await?;
    let dir = PathBuf::from(&rec.dir);
    let _ = tokio::fs::remove_dir_all(&dir).await;
    let bk = crate::backups::backups_root(&s.config.backups_dir(), &id);
    let _ = tokio::fs::remove_dir_all(&bk).await;
    s.db.audit(&user.username, "server_delete", &rec.name)
        .await
        .ok();
    Ok(Json(serde_json::json!({"ok": true})))
}

macro_rules! lifecycle {
    ($name:ident, $method:ident, $action:literal) => {
        pub async fn $name(
            State(s): State<Arc<AppState>>,
            user: AuthUser,
            Path(id): Path<String>,
        ) -> ApiResult<Json<serde_json::Value>> {
            if s.db.get_server(&id).await?.is_none() {
                return Err(ApiError::not_found("server not found"));
            }
            s.manager
                .$method(&id)
                .await
                .map_err(|e| ApiError::bad_request(e.to_string()))?;
            s.db.audit(&user.username, $action, &id).await.ok();
            Ok(Json(serde_json::json!({"ok": true})))
        }
    };
}

lifecycle!(start, start, "server_start");
lifecycle!(stop, stop, "server_stop");
lifecycle!(restart, restart, "server_restart");
lifecycle!(kill, kill, "server_kill");

#[derive(Deserialize)]
pub struct CommandRequest {
    pub command: String,
}

pub async fn command(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<CommandRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    if req.command.trim().is_empty() {
        return Err(ApiError::bad_request("empty command"));
    }
    s.manager
        .send_command(&id, req.command.trim_start_matches('/'))
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// Re-download the latest server jar/build.
pub async fn update_jar(
    State(s): State<Arc<AppState>>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let rt = s
        .manager
        .runtime(&id)
        .await
        .map_err(|_| ApiError::not_found("server not found"))?;
    if rt.status().await.is_active() {
        return Err(ApiError::conflict("stop the server before updating"));
    }
    crate::install::update_server(&s.http, &s.catalog, &rt)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    s.db.audit(&user.username, "server_update_jar", &id)
        .await
        .ok();
    Ok(Json(serde_json::json!({"ok": true})))
}

// ---------- server.properties ----------

pub async fn get_properties(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let rec = server_record(&s, &id).await?;
    let p = Properties::load(&PathBuf::from(&rec.dir).join("server.properties")).await?;
    let props: serde_json::Map<String, serde_json::Value> = p
        .entries()
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect();
    Ok(Json(serde_json::json!({"properties": props})))
}

pub async fn put_properties(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Map<String, serde_json::Value>>,
) -> ApiResult<Json<serde_json::Value>> {
    let rec = server_record(&s, &id).await?;
    let path = PathBuf::from(&rec.dir).join("server.properties");
    let mut p = Properties::load(&path).await?;
    let pairs: Vec<(String, String)> = body
        .iter()
        .map(|(k, v)| {
            let vs = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            (k.clone(), vs)
        })
        .collect();
    p.apply(&pairs);
    p.save(&path).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub async fn get_properties_raw(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<String> {
    let rec = server_record(&s, &id).await?;
    let p = Properties::load(&PathBuf::from(&rec.dir).join("server.properties")).await?;
    Ok(p.serialize())
}

pub async fn put_properties_raw(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    body: String,
) -> ApiResult<Json<serde_json::Value>> {
    let rec = server_record(&s, &id).await?;
    Properties::parse(&body)
        .save(&PathBuf::from(&rec.dir).join("server.properties"))
        .await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

// ---------- icon + tailscale ----------

/// Upload a server icon (PNG, ≤256×256 recommended, stored as server-icon.png).
pub async fn set_icon(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> ApiResult<Json<serde_json::Value>> {
    let rec = server_record(&s, &id).await?;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("icon") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| ApiError::bad_request(e.to_string()))?;
            if bytes.len() > 2 * 1024 * 1024 {
                return Err(ApiError::bad_request("icon too large (2 MiB max)"));
            }
            let path = PathBuf::from(&rec.dir).join("server-icon.png");
            tokio::fs::write(&path, &bytes).await?;
            return Ok(Json(serde_json::json!({"ok": true})));
        }
    }
    Err(ApiError::bad_request("missing 'icon' field"))
}

#[derive(Deserialize)]
pub struct TailscaleTcp {
    pub enable: bool,
    #[serde(default)]
    pub tailnet_port: Option<u16>,
}

/// Toggle `tailscale serve --tcp` exposure of this server's game port.
pub async fn tailscale_tcp(
    State(s): State<Arc<AppState>>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<TailscaleTcp>,
) -> ApiResult<Json<serde_json::Value>> {
    let rec = server_record(&s, &id).await?;
    let key = crate::tailscale::server_tcp_key(&id);
    if req.enable {
        let tport = req.tailnet_port.unwrap_or(rec.port as u16);
        s.tailscale
            .serve_tcp(rec.port as u16, tport)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        s.db.set_setting(&key, &tport.to_string()).await?;
        s.db.audit(
            &user.username,
            "tailscale_serve",
            &format!("{id} tcp:{tport}"),
        )
        .await
        .ok();
        Ok(Json(serde_json::json!({"ok": true, "tailnet_port": tport})))
    } else {
        let tport =
            s.db.get_setting(&key)
                .await?
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(rec.port as u16);
        s.tailscale
            .unserve_tcp(tport)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        s.db.delete_setting(&key).await?;
        Ok(Json(serde_json::json!({"ok": true})))
    }
}

async fn server_record(s: &AppState, id: &str) -> ApiResult<ServerRecord> {
    s.db.get_server(id)
        .await?
        .ok_or_else(|| ApiError::not_found("server not found"))
}
