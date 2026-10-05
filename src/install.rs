//! Server installation pipeline: download jars/installers, write eula.txt
//! and server.properties, run Forge/NeoForge installers headless.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

use crate::servers::props::{defaults, Properties};
use crate::servers::Runtime;
use crate::versions::{DownloadPlan, VersionCatalog};

/// Run the full install for a freshly created server record. Streams
/// progress lines into the server console (`rt`).
pub async fn install_server(
    http: &reqwest::Client,
    catalog: &VersionCatalog,
    rt: &Arc<Runtime>,
) -> Result<()> {
    let rec = rt.record.read().await.clone();
    let dir = PathBuf::from(&rec.dir);
    tokio::fs::create_dir_all(&dir).await?;

    log(rt, format!("[mcst] installing {} {} into {}", rec.server_type, rec.mc_version, dir.display())).await;

    let plan = catalog
        .plan(&rec.server_type, &rec.mc_version, &rec.loader_version)
        .await
        .context("failed to resolve download")?;

    match plan {
        DownloadPlan::Jar { url } => {
            log(rt, format!("[mcst] downloading {url}")).await;
            download(http, &url, &dir.join(&rec.jar)).await?;
        }
        DownloadPlan::Installer { url, filename } => {
            let installer = dir.join(&filename);
            log(rt, format!("[mcst] downloading {url}")).await;
            download(http, &url, &installer).await?;
            log(rt, "[mcst] running installer (this can take a minute)".into()).await;
            run_installer(&rec.java_path, &installer, &dir, rt).await?;
            let _ = tokio::fs::remove_file(&installer).await;
        }
        DownloadPlan::None => {
            log(rt, "[mcst] custom server — upload a jar named server.jar via Files".into()).await;
        }
    }

    // eula.txt — the create wizard collected the user's acceptance.
    tokio::fs::write(dir.join("eula.txt"), "eula=true\n").await?;

    // server.properties (keep any the installer already wrote).
    let props_path = dir.join("server.properties");
    if props_path.exists() {
        let mut p = Properties::load(&props_path).await?;
        p.set("server-port", &rec.port.to_string());
        p.save(&props_path).await?;
    } else {
        defaults(&rec.name, rec.port).save(&props_path).await?;
    }

    log(rt, "[mcst] install complete — press Start".into()).await;
    Ok(())
}

async fn log(rt: &Arc<Runtime>, line: String) {
    rt.push_log(line).await;
}

/// Stream a URL to disk.
pub async fn download(http: &reqwest::Client, url: &str, dest: &Path) -> Result<u64> {
    let res = http.get(url).send().await?.error_for_status()?;
    let mut f = tokio::fs::File::create(dest).await?;
    let mut stream = res.bytes_stream();
    let mut total = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        f.write_all(&chunk).await?;
        total += chunk.len() as u64;
    }
    f.flush().await?;
    Ok(total)
}

/// Run a Forge/NeoForge installer jar headless.
async fn run_installer(java: &str, installer: &Path, dir: &Path, rt: &Arc<Runtime>) -> Result<()> {
    let mut child = tokio::process::Command::new(java)
        .arg("-jar")
        .arg(installer)
        .arg("--installServer")
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to launch installer — is Java installed?")?;

    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let mut lines = tokio::io::BufReader::new(out).lines();
    let mut elines = tokio::io::BufReader::new(err).lines();
    use tokio::io::AsyncBufReadExt;
    loop {
        tokio::select! {
            l = lines.next_line() => match l {
                Ok(Some(l)) => log(rt, format!("[installer] {l}")).await,
                _ => break,
            },
            l = elines.next_line() => match l {
                Ok(Some(l)) => log(rt, format!("[installer] {l}")).await,
                Ok(None) => {}
                Err(_) => {}
            },
        }
    }
    let status = child.wait().await?;
    if !status.success() {
        bail!("installer exited with {status}");
    }
    Ok(())
}

/// Re-download the latest build for the server's current type/version —
/// the "Update" button. Keeps world/configs; replaces only the jar.
pub async fn update_server(
    http: &reqwest::Client,
    catalog: &VersionCatalog,
    rt: &Arc<Runtime>,
) -> Result<()> {
    let rec = rt.record.read().await.clone();
    if rt.status().await.is_active() {
        bail!("stop the server before updating");
    }
    match rec.server_type.as_str() {
        "paper" | "purpur" | "fabric" | "vanilla" => {}
        "forge" | "neoforge" => {
            bail!("forge/neoforge updates must be re-installed — update the loader version in settings, then use reinstall")
        }
        _ => bail!("unsupported server type for update"),
    }
    install_server(http, catalog, rt).await
}
