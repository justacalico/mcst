//! mcst — self-hosted Minecraft server panel. One binary, no env files.

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use mcst::{config, db, lock, AppState};

#[tokio::main]
async fn main() -> Result<()> {
    let mut cfg = match config::Config::from_args(std::env::args_os().skip(1))? {
        config::ConfigAction::Help => {
            println!("{}", config::usage());
            return Ok(());
        }
        config::ConfigAction::Version => {
            println!("mcst {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        config::ConfigAction::Run(c) => c,
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mcst=info,tower_http=warn".into()),
        )
        .init();

    if cfg.dev_mode {
        cfg.apply_dev_mode();
    }

    // Everything lives under the data dir.
    for d in [
        cfg.data_dir.clone(),
        cfg.servers_dir(),
        cfg.backups_dir(),
        cfg.cache_dir(),
        cfg.java_dir(),
    ] {
        tokio::fs::create_dir_all(&d).await?;
    }

    let _guard = if cfg.dev_mode {
        None
    } else {
        Some(lock::SingleInstance::acquire(&cfg.data_dir)?)
    };

    let bind = cfg.bind_addr();
    let database = db::Db::connect(&cfg.db_url()).await?;

    let http = reqwest::Client::builder()
        .user_agent(concat!("mcst/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let manager = mcst::servers::ServerManager::new(database.clone(), cfg.clone());
    let state = Arc::new(AppState {
        config: Arc::new(cfg.clone()),
        db: database.clone(),
        manager: manager.clone(),
        catalog: mcst::versions::VersionCatalog::new(http.clone()),
        tailscale: mcst::tailscale::Tailscale::new(cfg.tailscale_bin.clone()),
        http,
        sysstats_cache: RwLock::new(None),
    });

    manager.start_supervisor();
    manager.spawn_stats_loop();
    manager.spawn_empty_stop_loop();
    mcst::schedules::spawn_loop(manager.clone(), database.clone());
    if !cfg.dev_mode {
        manager.autostart().await;
    }

    let app = mcst::build_app(state.clone());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    let addr = listener.local_addr()?;

    if cfg.dev_mode {
        tracing::info!("--dev: serving http://{addr} with no auth and a throwaway database");
    } else {
        let count = database.user_count().await.unwrap_or(0);
        if count == 0 {
            tracing::info!("first run — open http://{addr} to create your account");
        }
        tracing::info!(%addr, "mcst listening");
    }

    // Re-enable the stored tailscale panel serve setting, if any.
    if !cfg.dev_mode {
        restore_tailscale(&state).await;
    }

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    // Stop all managed servers so no java processes are orphaned.
    manager.stop_all(std::time::Duration::from_secs(15)).await;
    Ok(())
}

/// Re-apply the persisted tailscale panel serve on startup.
async fn restore_tailscale(state: &Arc<AppState>) {
    use mcst::tailscale::{KEY_PANEL_SERVE, KEY_PANEL_SERVE_PORT};
    let enabled = state
        .db
        .get_setting(KEY_PANEL_SERVE)
        .await
        .ok()
        .flatten()
        .map(|v| v == "1")
        .unwrap_or(false);
    if !enabled {
        return;
    }
    let port = state
        .db
        .get_setting(KEY_PANEL_SERVE_PORT)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(mcst::tailscale::DEFAULT_HTTPS_PORT);
    if let Err(e) = state.tailscale.serve_panel(state.config.port, port).await {
        tracing::warn!("tailscale serve restore failed: {e}");
    } else {
        tracing::info!(https_port = port, "tailscale serve restored");
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down — stopping managed servers");
}
