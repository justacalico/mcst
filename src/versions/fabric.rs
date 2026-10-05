//! Fabric via meta.fabricmc.net.

use anyhow::{Context, Result};
use serde::Deserialize;

const META: &str = "https://meta.fabricmc.net/v2";

#[derive(Debug, Deserialize)]
struct GameVersion {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct LoaderVersion {
    #[serde(alias = "loader")]
    loader: Option<LoaderObj>,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoaderObj {
    version: String,
}

pub async fn list_game_versions(http: &reqwest::Client) -> Result<Vec<String>> {
    let v: Vec<GameVersion> = http
        .get(format!("{META}/versions/game"))
        .send()
        .await?
        .json()
        .await?;
    Ok(v.into_iter()
        .filter(|g| g.stable)
        .map(|g| g.version)
        .collect())
}

pub async fn list_loaders(http: &reqwest::Client) -> Result<Vec<String>> {
    let v: Vec<serde_json::Value> = http
        .get(format!("{META}/versions/loader"))
        .send()
        .await?
        .json()
        .await?;
    Ok(v.iter()
        .filter_map(|e| {
            e.get("loader")
                .and_then(|l| l.get("version"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect())
}

pub async fn latest_installer(http: &reqwest::Client) -> Result<String> {
    let v: Vec<LoaderVersion> = http
        .get(format!("{META}/versions/installer"))
        .send()
        .await?
        .json()
        .await?;
    v.first()
        .and_then(|e| {
            e.version
                .clone()
                .or_else(|| e.loader.as_ref().map(|l| l.version.clone()))
        })
        .context("no fabric installer versions")
}

/// Ready-to-run server jar URL for a game/loader/installer triple.
pub fn server_jar_url(mc: &str, loader: &str, installer: &str) -> String {
    format!("{META}/versions/loader/{mc}/{loader}/{installer}/server/jar")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jar_url() {
        assert_eq!(
            server_jar_url("1.21", "0.16.0", "1.0.1"),
            "https://meta.fabricmc.net/v2/versions/loader/1.21/0.16.0/1.0.1/server/jar"
        );
    }
}
