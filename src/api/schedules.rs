//! Schedule endpoints.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db::ScheduleRow;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

pub async fn list(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let rows = s.db.list_schedules(&id).await?;
    Ok(Json(serde_json::json!({"schedules": rows})))
}

#[derive(Deserialize)]
pub struct ScheduleReq {
    pub name: Option<String>,
    pub action: String,
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub every_minutes: i64,
    #[serde(default)]
    pub daily_time: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<ScheduleReq>,
) -> ApiResult<Json<serde_json::Value>> {
    if s.db.get_server(&id).await?.is_none() {
        return Err(ApiError::not_found("server not found"));
    }
    crate::schedules::validate(&req.action, &req.payload, req.every_minutes, &req.daily_time)
        .map_err(ApiError::bad_request)?;
    let row = ScheduleRow {
        id: Uuid::new_v4().to_string(),
        server_id: id,
        name: req.name.unwrap_or_else(|| req.action.clone()),
        action: req.action,
        payload: req.payload,
        every_minutes: req.every_minutes,
        daily_time: req.daily_time,
        enabled: req.enabled as i64,
        last_run_at: None,
        created_at: String::new(),
    };
    s.db.insert_schedule(&row).await?;
    Ok(Json(serde_json::json!({"schedule": row})))
}

pub async fn update(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path((_id, sid)): Path<(String, String)>,
    Json(req): Json<ScheduleReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut row = s
        .db
        .get_schedule(&sid)
        .await?
        .ok_or_else(|| ApiError::not_found("schedule not found"))?;
    crate::schedules::validate(&req.action, &req.payload, req.every_minutes, &req.daily_time)
        .map_err(ApiError::bad_request)?;
    row.name = req.name.unwrap_or(row.name);
    row.action = req.action;
    row.payload = req.payload;
    row.every_minutes = req.every_minutes;
    row.daily_time = req.daily_time;
    row.enabled = req.enabled as i64;
    s.db.update_schedule(&row).await?;
    Ok(Json(serde_json::json!({"schedule": row})))
}

pub async fn remove(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path((_id, sid)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    if s.db.get_schedule(&sid).await?.is_none() {
        return Err(ApiError::not_found("schedule not found"));
    }
    s.db.delete_schedule(&sid).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}
