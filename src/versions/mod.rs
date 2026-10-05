//! Version catalog: which MC versions + loader builds are installable for
//! each supported server type.

pub mod compare;
pub mod fabric;
pub mod maven;
pub mod mojang;
pub mod paper;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::servers::types::ServerType;

#[derive(Clone)]
pub struct VersionCatalog {
    http: reqwest::Client,
}

/// What an install needs to fetch/run.
#[derive(Debug, Clone)]
pub enum DownloadPlan {
    /// Download a ready-to-run jar to `server.jar`.
    Jar { url: String },
    /// Download an installer jar then run `java -jar <file> --installServer`.
    Installer { url: String, filename: String },
    /// No download — a `custom` server the user fills in themselves.
    None,
}

#[derive(Debug, Serialize)]
pub struct TypeInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub has_loader: bool,
    pub content_dir: &'static str,
}

pub fn type_infos() -> Vec<TypeInfo> {
    vec![
        TypeInfo { id: "vanilla", name: "Vanilla", has_loader: false, content_dir: "mods" },
        TypeInfo { id: "paper", name: "Paper", has_loader: false, content_dir: "plugins" },
        TypeInfo { id: "purpur", name: "Purpur", has_loader: false, content_dir: "plugins" },
        TypeInfo { id: "fabric", name: "Fabric", has_loader: true, content_dir: "mods" },
        TypeInfo { id: "forge", name: "Forge", has_loader: true, content_dir: "mods" },
        TypeInfo { id: "neoforge", name: "NeoForge", has_loader: true, content_dir: "mods" },
        TypeInfo { id: "custom", name: "Custom jar", has_loader: false, content_dir: "mods" },
    ]
}

impl VersionCatalog {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// MC versions available for a server type, newest first.
    pub async fn mc_versions(&self, server_type: &str) -> Result<Vec<String>> {
        let t = ServerType::parse(server_type)
            .with_context(|| format!("unknown server type '{server_type}'"))?;
        match t {
            ServerType::Vanilla => mojang::list(&self.http).await,
            ServerType::Paper => paper::list(&self.http, "paper").await,
            ServerType::Purpur => paper::list(&self.http, "purpur").await,
            ServerType::Fabric => fabric::list_game_versions(&self.http).await,
            ServerType::Forge => {
                let v = maven::fetch_maven_versions(&self.http, maven::FORGE_MAVEN).await?;
                Ok(maven::forge_mc_versions(&v))
            }
            ServerType::NeoForge => {
                let v = maven::fetch_maven_versions(&self.http, maven::NEOFORGE_MAVEN).await?;
                Ok(maven::neoforge_mc_versions(&v))
            }
            ServerType::Custom => Ok(vec![]),
        }
    }

    /// Loader/build choices for `(type, mc_version)`. Empty when the type has
    /// no loader dimension.
    pub async fn loaders(&self, server_type: &str, mc: &str) -> Result<Vec<String>> {
        let t = ServerType::parse(server_type)
            .with_context(|| format!("unknown server type '{server_type}'"))?;
        match t {
            ServerType::Fabric => fabric::list_loaders(&self.http).await,
            ServerType::Forge => {
                let v = maven::fetch_maven_versions(&self.http, maven::FORGE_MAVEN).await?;
                Ok(maven::forge_loaders_for(&v, mc))
            }
            ServerType::NeoForge => {
                let v = maven::fetch_maven_versions(&self.http, maven::NEOFORGE_MAVEN).await?;
                Ok(maven::neoforge_for_mc(&v, mc))
            }
            ServerType::Paper | ServerType::Purpur => {
                // Surface build numbers as the "loader" choice.
                let b = paper::latest_build(&self.http, server_type, mc).await?;
                Ok(vec![b.to_string()])
            }
            _ => Ok(vec![]),
        }
    }

    /// Resolve an install into a download plan.
    pub async fn plan(
        &self,
        server_type: &str,
        mc: &str,
        loader: &str,
    ) -> Result<DownloadPlan> {
        let t = ServerType::parse(server_type)
            .with_context(|| format!("unknown server type '{server_type}'"))?;
        match t {
            ServerType::Vanilla => {
                let url = mojang::server_jar_url(&self.http, mc).await?;
                Ok(DownloadPlan::Jar { url })
            }
            ServerType::Paper | ServerType::Purpur => {
                let build = if loader.is_empty() {
                    paper::latest_build(&self.http, server_type, mc).await?
                } else {
                    loader.parse::<i64>().context("invalid build number")?
                };
                Ok(DownloadPlan::Jar {
                    url: paper::download_url(server_type, mc, build),
                })
            }
            ServerType::Fabric => {
                if loader.is_empty() {
                    bail!("fabric requires a loader version");
                }
                let installer = fabric::latest_installer(&self.http).await?;
                Ok(DownloadPlan::Jar {
                    url: fabric::server_jar_url(mc, loader, &installer),
                })
            }
            ServerType::Forge => {
                if loader.is_empty() {
                    bail!("forge requires a loader version");
                }
                Ok(DownloadPlan::Installer {
                    url: maven::forge_installer_url(loader),
                    filename: format!("forge-{loader}-installer.jar"),
                })
            }
            ServerType::NeoForge => {
                if loader.is_empty() {
                    bail!("neoforge requires a loader version");
                }
                Ok(DownloadPlan::Installer {
                    url: maven::neoforge_installer_url(loader),
                    filename: format!("neoforge-{loader}-installer.jar"),
                })
            }
            ServerType::Custom => Ok(DownloadPlan::None),
        }
    }
}
