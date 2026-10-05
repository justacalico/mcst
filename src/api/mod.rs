//! HTTP API surface. All routes are mounted under `/api`.

pub mod audit;
pub mod auth;
pub mod backups;
pub mod console;
pub mod events;
pub mod files;
pub mod java;
pub mod mods;
pub mod players;
pub mod schedules;
pub mod servers;
pub mod settings;
pub mod setup;
pub mod system;
pub mod tailscale;
pub mod versions;

use std::sync::Arc;

use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    let authed = Router::new()
        // auth/session
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/password", post(auth::change_password))
        // system + events
        .route("/system", get(system::system_stats))
        .route("/events", get(events::events_ws))
        // servers
        .route("/servers", get(servers::list).post(servers::create))
        .route(
            "/servers/{id}",
            get(servers::get)
                .patch(servers::update)
                .delete(servers::delete),
        )
        .route("/servers/{id}/start", post(servers::start))
        .route("/servers/{id}/stop", post(servers::stop))
        .route("/servers/{id}/restart", post(servers::restart))
        .route("/servers/{id}/kill", post(servers::kill))
        .route("/servers/{id}/update", post(servers::update_jar))
        .route(
            "/servers/{id}/icon",
            post(servers::set_icon)
                .route_layer(axum::extract::DefaultBodyLimit::max(3 * 1024 * 1024)),
        )
        .route("/servers/{id}/console", get(console::console_ws))
        .route("/servers/{id}/command", post(servers::command))
        .route(
            "/servers/{id}/properties",
            get(servers::get_properties).put(servers::put_properties),
        )
        .route(
            "/servers/{id}/properties/raw",
            get(servers::get_properties_raw).put(servers::put_properties_raw),
        )
        .route("/servers/{id}/tailscale", post(servers::tailscale_tcp))
        // files
        .route("/servers/{id}/files", get(files::list))
        .route("/servers/{id}/files/read", get(files::read))
        .route("/servers/{id}/files/write", put(files::write))
        .route("/servers/{id}/files/upload", post(files::upload))
        .route("/servers/{id}/files/download", get(files::download))
        .route("/servers/{id}/files/mkdir", post(files::mkdir))
        .route("/servers/{id}/files/rename", post(files::rename))
        .route("/servers/{id}/files/delete", post(files::delete_file))
        // backups
        .route(
            "/servers/{id}/backups",
            get(backups::list).post(backups::create),
        )
        .route(
            "/servers/{id}/backups/{bid}/restore",
            post(backups::restore),
        )
        .route(
            "/servers/{id}/backups/{bid}/download",
            get(backups::download),
        )
        .route("/servers/{id}/backups/{bid}", delete(backups::remove))
        // players
        .route(
            "/servers/{id}/players/{list}",
            get(players::list).post(players::add),
        )
        .route(
            "/servers/{id}/players/{list}/{name}",
            delete(players::remove),
        )
        // schedules
        .route(
            "/servers/{id}/schedules",
            get(schedules::list).post(schedules::create),
        )
        .route(
            "/servers/{id}/schedules/{sid}",
            patch(schedules::update).delete(schedules::remove),
        )
        // mods/plugins
        .route("/servers/{id}/mods/search", get(mods::search))
        .route("/servers/{id}/mods/install", post(mods::install))
        .route("/servers/{id}/mods", get(mods::installed))
        // versions catalog
        .route("/versions", get(versions::mc_versions))
        .route("/versions/loaders", get(versions::loaders))
        .route("/versions/types", get(versions::types))
        // java
        .route("/java", get(java::list).post(java::install))
        .route("/java/{id}", delete(java::remove))
        .route("/java/required", get(java::required))
        // tailscale
        .route(
            "/tailscale",
            get(tailscale::status).post(tailscale::panel_serve),
        )
        // settings + audit
        .route("/settings", get(settings::get_all).put(settings::update))
        .route("/audit", get(audit::list))
        .layer(RequestBodyLimitLayer::new(512 * 1024 * 1024));

    Router::new()
        // public
        .route("/setup/status", get(setup::status))
        .route("/setup", post(setup::setup))
        .route("/auth/login", post(auth::login))
        .route("/healthz", get(|| async { "ok" }))
        .merge(authed)
        .layer(TraceLayer::new_for_http())
}
