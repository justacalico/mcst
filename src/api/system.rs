//! Host system stats endpoint.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::Json;

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::AppState;

/// Cached for ~2s — `collect` itself takes ~200ms for the CPU sample.
pub async fn system_stats(
    State(s): State<Arc<AppState>>,
    _u: AuthUser,
) -> ApiResult<Json<serde_json::Value>> {
    const TTL: Duration = Duration::from_secs(2);
    {
        let cache = s.sysstats_cache.read().await;
        if let Some((at, stats)) = cache.as_ref() {
            if at.elapsed() < TTL {
                return Ok(Json(serde_json::to_value(stats).unwrap_or_default()));
            }
        }
    }
    let stats = crate::sysstats::collect(&s.config.data_dir).await;
    *s.sysstats_cache.write().await = Some((std::time::Instant::now(), stats.clone()));
    Ok(Json(serde_json::to_value(stats).unwrap_or_default()))
}
