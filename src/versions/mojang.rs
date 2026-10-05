//! Vanilla Minecraft versions via Mojang's piston-meta manifest.

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::versions::compare;

const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Deserialize)]
struct Manifest {
    versions: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct ManifestEntry {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    url: String,
}

/// MC versions: releases only, newest first.
pub async fn list(http: &reqwest::Client) -> Result<Vec<String>> {
    let m: Manifest = http.get(MANIFEST).send().await?.json().await?;
    let mut v: Vec<String> = m
        .versions
        .iter()
        .filter(|e| e.kind == "release")
        .map(|e| e.id.clone())
        .collect();
    compare::sort_desc(&mut v);
    Ok(v)
}

/// Resolve the server jar URL for a vanilla version: manifest → version doc
/// → `downloads.server.url`.
pub async fn server_jar_url(http: &reqwest::Client, mc_version: &str) -> Result<String> {
    let m: Manifest = http.get(MANIFEST).send().await?.json().await?;
    let entry = m
        .versions
        .iter()
        .find(|e| e.id == mc_version)
        .with_context(|| format!("unknown Minecraft version {mc_version}"))?;
    let doc: serde_json::Value = http.get(&entry.url).send().await?.json().await?;
    doc.pointer("/downloads/server/url")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .with_context(|| format!("no server jar listed for {mc_version}"))
}

/// Java major version required by a vanilla release, per Mojang's docs:
/// ≤1.16.5 → 8, 1.17.x → 16, 1.18–1.20.4 → 17, 1.20.5+ → 21.
pub fn required_java(mc_version: &str) -> u32 {
    let parts: Vec<u64> = mc_version
        .split('.')
        .map(|p| p.parse::<u64>().unwrap_or(u64::MAX))
        .collect();
    let minor = parts.get(1).copied().unwrap_or(u64::MAX);
    let patch = parts.get(2).copied().unwrap_or(0);
    match minor {
        0..=16 => 8,
        17 => 16,
        18..=20 => {
            if minor == 20 && patch >= 5 {
                21
            } else {
                17
            }
        }
        _ => 21,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_requirements() {
        assert_eq!(required_java("1.8.8"), 8);
        assert_eq!(required_java("1.12.2"), 8);
        assert_eq!(required_java("1.16.5"), 8);
        assert_eq!(required_java("1.17.1"), 16);
        assert_eq!(required_java("1.18.2"), 17);
        assert_eq!(required_java("1.20.4"), 17);
        assert_eq!(required_java("1.20.5"), 21);
        assert_eq!(required_java("1.21"), 21);
        assert_eq!(required_java("1.21.4"), 21);
        // Snapshots/unknown → newest.
        assert_eq!(required_java("24w14a"), 21);
        assert_eq!(required_java("1.99"), 21);
    }
}
