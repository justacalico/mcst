//! Version catalog endpoints feeding the create wizard.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Deserialize)]
pub struct VersionsQuery {
    #[serde(rename = "type")]
    pub server_type: String,
}

pub async fn types(_u: AuthUser) -> Json<serde_json::Value> {
    Json(serde_json::json!({"types": crate::versions::type_infos()}))
}

pub async fn mc_versions(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Query(q): Query<VersionsQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let v = s
        .catalog
        .mc_versions(&q.server_type)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(serde_json::json!({"versions": v})))
}

#[derive(Deserialize)]
pub struct LoadersQuery {
    #[serde(rename = "type")]
    pub server_type: String,
    pub mc_version: String,
}

pub async fn loaders(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
    Query(q): Query<LoadersQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let v = s
        .catalog
        .loaders(&q.server_type, &q.mc_version)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(serde_json::json!({"loaders": v})))
}
