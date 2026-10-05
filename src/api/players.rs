//! Player list endpoints: ops / whitelist / bans.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::players::ListKind;
use crate::AppState;

async fn dir_of(s: &AppState, id: &str) -> ApiResult<PathBuf> {
    s.db.get_server(id)
        .await?
        .map(|r| PathBuf::from(r.dir))
        .ok_or_else(|| ApiError::not_found("server not found"))
}

pub async fn list(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path((id, list)): Path<(String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let kind = ListKind::parse(&list).ok_or_else(|| ApiError::bad_request("unknown list"))?;
    let dir = dir_of(&s, &id).await?;
    let entries = crate::players::list(&dir, kind).await?;
    Ok(Json(serde_json::json!({"players": entries})))
}

#[derive(Deserialize)]
pub struct AddReq {
    pub name: String,
}

pub async fn add(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path((id, list)): Path<(String, String)>,
    Json(req): Json<AddReq>,
) -> ApiResult<Json<serde_json::Value>> {
    let kind = ListKind::parse(&list).ok_or_else(|| ApiError::bad_request("unknown list"))?;
    let dir = dir_of(&s, &id).await?;
    let entries = crate::players::add(&s.manager, &dir, &id, kind, &req.name, &s.http).await?;
    Ok(Json(serde_json::json!({"players": entries})))
}

pub async fn remove(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Path((id, list, name)): Path<(String, String, String)>,
) -> ApiResult<Json<serde_json::Value>> {
    let kind = ListKind::parse(&list).ok_or_else(|| ApiError::bad_request("unknown list"))?;
    let dir = dir_of(&s, &id).await?;
    let entries = crate::players::remove(&s.manager, &dir, &id, kind, &name).await?;
    Ok(Json(serde_json::json!({"players": entries})))
}
