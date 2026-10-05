//! Audit log endpoint.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::AppState;

pub async fn list(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
) -> ApiResult<Json<serde_json::Value>> {
    let rows = s.db.audit_log(200).await?;
    Ok(Json(serde_json::json!({"entries": rows})))
}
