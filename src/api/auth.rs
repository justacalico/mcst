//! Login/logout/me/password endpoints.

use std::sync::Arc;

use axum::extract::State;
use axum::http::header;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::{password, token_from_parts, AuthUser, SESSION_COOKIE};
use crate::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub id: String,
    pub username: String,
    pub dev_mode: bool,
}

fn session_cookie(token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        30 * 24 * 3600
    )
}

pub async fn login(
    State(s): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> ApiResult<impl axum::response::IntoResponse> {
    let user = s.db.user_by_name(&req.username).await?;
    // Spend equivalent hashing time on a miss so the response timing doesn't
    // reveal whether the username exists.
    let Some(user) = user else {
        let _ = password::hash(&req.password);
        return Err(ApiError::unauthorized("invalid username or password"));
    };
    if !password::verify(&req.password, &user.password_hash) {
        return Err(ApiError::unauthorized("invalid username or password"));
    }
    let token = crate::auth::create_session(&s.db, &user.id).await?;
    s.db.audit(&user.username, "login", "").await.ok();
    Ok((
        [(header::SET_COOKIE, session_cookie(&token))],
        Json(serde_json::json!({"ok": true, "username": user.username})),
    ))
}

pub async fn logout(
    State(s): State<Arc<AppState>>,
    parts: axum::http::request::Parts,
) -> ApiResult<impl axum::response::IntoResponse> {
    if let Some(t) = token_from_parts(&parts) {
        s.db.delete_session(&t).await.ok();
    }
    Ok((
        [(
            header::SET_COOKIE,
            format!("{SESSION_COOKIE}=; Path=/; HttpOnly; Max-Age=0"),
        )],
        Json(serde_json::json!({"ok": true})),
    ))
}

pub async fn me(user: AuthUser, State(s): State<Arc<AppState>>) -> Json<MeResponse> {
    Json(MeResponse {
        id: user.id,
        username: user.username,
        dev_mode: s.config.dev_mode,
    })
}

#[derive(Deserialize)]
pub struct PasswordChange {
    pub current: String,
    pub new_password: String,
}

pub async fn change_password(
    user: AuthUser,
    State(s): State<Arc<AppState>>,
    Json(req): Json<PasswordChange>,
) -> ApiResult<Json<serde_json::Value>> {
    let row =
        s.db.user_by_id(&user.id)
            .await?
            .ok_or_else(|| ApiError::unauthorized("no account"))?;
    if !password::verify(&req.current, &row.password_hash) {
        return Err(ApiError::forbidden("current password is wrong"));
    }
    password::validate(&req.new_password).map_err(ApiError::bad_request)?;
    let hash = password::hash(&req.new_password).map_err(|e| ApiError::internal(e.to_string()))?;
    s.db.set_password(&user.id, &hash).await?;
    s.db.delete_user_sessions(&user.id).await.ok();
    s.db.audit(&user.username, "password_change", "").await.ok();
    Ok(Json(serde_json::json!({"ok": true})))
}
