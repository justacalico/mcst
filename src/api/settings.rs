//! Panel settings (key/value store).

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

/// Settings the UI can read/write. Anything else is refused — the settings
/// table also holds internal keys (tailscale state) we don't expose raw.
const PUBLIC_KEYS: &[&str] = &[
    "theme",
    "default_memory_mb",
    "default_shutdown_timeout_sec",
    "backup_keep",
    "panel_name",
];

pub async fn get_all(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
) -> ApiResult<Json<serde_json::Value>> {
    let mut out = serde_json::Map::new();
    for k in PUBLIC_KEYS {
        if let Some(v) = s.db.get_setting(k).await? {
            out.insert(k.to_string(), serde_json::Value::String(v));
        }
    }
    Ok(Json(serde_json::json!({"settings": out})))
}

pub async fn update(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Json(body): Json<serde_json::Map<String, serde_json::Value>>,
) -> ApiResult<Json<serde_json::Value>> {
    for (k, v) in body.iter() {
        if !PUBLIC_KEYS.contains(&k.as_str()) {
            return Err(ApiError::bad_request(format!("unknown setting '{k}'")));
        }
        let val = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if val.len() > 4096 {
            return Err(ApiError::bad_request("setting value too long"));
        }
        s.db.set_setting(k, &val).await?;
    }
    Ok(Json(serde_json::json!({"ok": true})))
}
