//! First-run setup: create the owner account when none exists.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::{password, SESSION_COOKIE};
use crate::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Serialize)]
pub struct SetupStatus {
    pub needs_setup: bool,
    pub dev_mode: bool,
}

pub async fn status(State(s): State<Arc<AppState>>) -> ApiResult<Json<SetupStatus>> {
    let count =
        s.db.user_count()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(SetupStatus {
        needs_setup: count == 0 && !s.config.dev_mode,
        dev_mode: s.config.dev_mode,
    }))
}

#[derive(Deserialize)]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
}

pub async fn setup(
    State(s): State<Arc<AppState>>,
    Json(req): Json<SetupRequest>,
) -> ApiResult<impl axum::response::IntoResponse> {
    if s.db.user_count().await? > 0 {
        return Err(ApiError::conflict("setup already completed"));
    }
    password::validate_username(&req.username).map_err(ApiError::bad_request)?;
    password::validate(&req.password).map_err(ApiError::bad_request)?;
    let hash = password::hash(&req.password).map_err(|e| ApiError::internal(e.to_string()))?;
    let uid = s.db.create_user(&req.username, &hash).await?;
    let token = crate::auth::create_session(&s.db, &uid).await?;
    s.db.audit(&req.username, "setup", "owner account created")
        .await
        .ok();
    let cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        30 * 24 * 3600
    );
    Ok((
        [(axum::http::header::SET_COOKIE, cookie)],
        Json(serde_json::json!({"ok": true, "username": req.username})),
    ))
}
