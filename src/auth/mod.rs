//! Authentication: argon2 password hashing, session tokens, extractor.

use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;

use crate::db::{Db, UserRow};
use crate::error::ApiError;
use crate::AppState;

pub mod password;

pub const SESSION_COOKIE: &str = "mcst_session";
const SESSION_DAYS: i64 = 30;

/// A successfully authenticated request.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
}

/// Mint a session token and store it.
pub async fn create_session(db: &Db, user_id: &str) -> anyhow::Result<String> {
    let token = random_token(32);
    db.create_session(user_id, &token, SESSION_DAYS).await?;
    Ok(token)
}

fn random_token(n: usize) -> String {
    use rand::RngCore;
    let mut buf = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Extract the session token from the `mcst_session` cookie or an
/// `Authorization: Bearer <token>` header.
pub fn token_from_parts(parts: &Parts) -> Option<String> {
    if let Some(h) = parts.headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(v) = h.to_str() {
            if let Some(t) = v.strip_prefix("Bearer ") {
                return Some(t.trim().to_string());
            }
        }
    }
    let jar = CookieJar::from_headers(&parts.headers);
    jar.get(SESSION_COOKIE).map(|c| c.value().to_string())
}

/// Auth extractor: resolves the session cookie/bearer to a user. In dev
/// mode every request is the implicit `dev` account.
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let state = state.clone();
        let token = token_from_parts(parts);
        async move {
            if state.config.dev_mode {
                return Ok(AuthUser {
                    id: "dev".into(),
                    username: "dev".into(),
                });
            }
            let token = token.ok_or_else(|| ApiError::unauthorized("not logged in"))?;
            let user: Option<UserRow> = state
                .db
                .session_user(&token)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            let user = user.ok_or_else(|| ApiError::unauthorized("session expired"))?;
            Ok(AuthUser {
                id: user.id,
                username: user.username,
            })
        }
    }
}
