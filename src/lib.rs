//! mcst — a self-hosted, single-binary panel for creating and managing
//! Minecraft servers.

pub mod api;
pub mod assets;
pub mod auth;
pub mod backups;
pub mod config;
pub mod db;
pub mod error;
pub mod files;
pub mod install;
pub mod java;
pub mod lock;
pub mod modrinth;
pub mod ping;
pub mod players;
pub mod rcon;
pub mod schedules;
pub mod servers;
pub mod sysstats;
pub mod tailscale;
pub mod versions;

use std::sync::Arc;
use std::time::Instant;

use axum::routing::get;
use axum::Router;
use tokio::sync::RwLock;
use tower_http::compression::CompressionLayer;

use crate::config::Config;
use crate::db::Db;
use crate::servers::ServerManager;
use crate::sysstats::SystemStats;
use crate::versions::VersionCatalog;

/// Shared application state.
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Db,
    pub manager: ServerManager,
    pub catalog: VersionCatalog,
    pub tailscale: tailscale::Tailscale,
    pub http: reqwest::Client,
    /// Short-TTL cache for the system stats endpoint.
    pub sysstats_cache: RwLock<Option<(Instant, SystemStats)>>,
}

/// Build the complete axum app (API + embedded frontend).
pub fn build_app(state: Arc<AppState>) -> Router {
    Router::<Arc<AppState>>::new()
        .nest("/api", api::router())
        .route("/api/{*path}", get(api_not_found).post(api_not_found))
        .fallback(assets::serve)
        .layer(CompressionLayer::new())
        .with_state(state)
}

async fn api_not_found() -> crate::error::ApiError {
    crate::error::ApiError::not_found("unknown api endpoint")
}


