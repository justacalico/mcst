//! Tailscale endpoints: status + panel serve toggle.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::tailscale::{KEY_PANEL_SERVE, KEY_PANEL_SERVE_PORT};
use crate::AppState;

pub async fn status(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
) -> ApiResult<Json<serde_json::Value>> {
    let enabled =
        s.db.get_setting(KEY_PANEL_SERVE)
            .await?
            .map(|v| v == "1")
            .unwrap_or(false);
    let port =
        s.db.get_setting(KEY_PANEL_SERVE_PORT)
            .await?
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(crate::tailscale::DEFAULT_HTTPS_PORT);
    let st = s.tailscale.status(port).await;
    Ok(Json(serde_json::json!({
        "status": st,
        "panel_serve_enabled": enabled,
    })))
}

#[derive(Deserialize)]
pub struct ServeReq {
    pub enable: bool,
    #[serde(default)]
    pub https_port: Option<u16>,
}

pub async fn panel_serve(
    State(s): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<ServeReq>,
) -> ApiResult<Json<serde_json::Value>> {
    if !s.tailscale.installed().await {
        return Err(ApiError::bad_request(
            "tailscale is not installed on this host",
        ));
    }
    let https_port = req
        .https_port
        .unwrap_or(crate::tailscale::DEFAULT_HTTPS_PORT);
    if req.enable {
        s.tailscale
            .serve_panel(s.config.port, https_port)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        s.db.set_setting(KEY_PANEL_SERVE, "1").await?;
        s.db.set_setting(KEY_PANEL_SERVE_PORT, &https_port.to_string())
            .await?;
        s.db.audit(&user.username, "tailscale_panel_on", "")
            .await
            .ok();
    } else {
        let stored =
            s.db.get_setting(KEY_PANEL_SERVE_PORT)
                .await?
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(https_port);
        s.tailscale
            .unserve_panel(stored)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        s.db.set_setting(KEY_PANEL_SERVE, "0").await?;
        s.db.audit(&user.username, "tailscale_panel_off", "")
            .await
            .ok();
    }
    Ok(Json(serde_json::json!({"ok": true})))
}
