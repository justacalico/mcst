//! Modrinth integration: search and install mods/plugins into a server's
//! `mods/` or `plugins/` directory.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

const API: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub downloads: i64,
    pub icon_url: Option<String>,
    pub project_type: String,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    hits: Vec<Hit>,
}

#[derive(Debug, Deserialize)]
struct Hit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    downloads: i64,
    icon_url: Option<String>,
    project_type: String,
}

/// Search projects. `loader` e.g. "paper"/"fabric"; `project_type` is
/// "mod" or "plugin".
pub async fn search(
    http: &reqwest::Client,
    query: &str,
    loader: &str,
    project_type: &str,
    mc_version: &str,
    limit: usize,
) -> Result<Vec<SearchHit>> {
    let mut facets = format!("[[\"project_type:{project_type}\"]");
    if !loader.is_empty() {
        facets.push_str(&format!(",[\"categories:{loader}\"]"));
    }
    if !mc_version.is_empty() {
        facets.push_str(&format!(",[\"versions:{mc_version}\"]"));
    }
    facets.push(']');
    let res: SearchResponse = http
        .get(format!("{API}/search"))
        .query(&[
            ("query", query),
            ("facets", &facets),
            ("limit", &limit.min(50).to_string()),
            ("index", "downloads"),
        ])
        .send()
        .await?
        .json()
        .await?;
    Ok(res
        .hits
        .into_iter()
        .map(|h| SearchHit {
            project_id: h.project_id,
            slug: h.slug,
            title: h.title,
            description: h.description,
            downloads: h.downloads,
            icon_url: h.icon_url,
            project_type: h.project_type,
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct VersionFile {
    url: String,
    filename: String,
    primary: bool,
}

#[derive(Debug, Deserialize)]
struct VersionEntry {
    files: Vec<VersionFile>,
}

/// Install a project's best matching version into `dir`.
/// Returns the installed filename.
pub async fn install(
    http: &reqwest::Client,
    project: &str,
    loader: &str,
    mc_version: &str,
    dir: &Path,
) -> Result<String> {
    let mut query: Vec<(&str, String)> = vec![];
    if !loader.is_empty() {
        query.push(("loaders", format!("[\"{loader}\"]")));
    }
    if !mc_version.is_empty() {
        query.push(("game_versions", format!("[\"{mc_version}\"]")));
    }
    let versions: Vec<VersionEntry> = http
        .get(format!("{API}/project/{project}/version"))
        .query(&query)
        .send()
        .await?
        .json()
        .await?;
    let file = versions
        .iter()
        .flat_map(|v| v.files.iter())
        .find(|f| f.primary)
        .or_else(|| versions.iter().flat_map(|v| v.files.iter()).next())
        .context("no downloadable file found for this project")?;
    let bytes = http
        .get(&file.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    tokio::fs::create_dir_all(dir).await?;
    let name = sanitize_filename(&file.filename);
    tokio::fs::write(dir.join(&name), &bytes).await?;
    Ok(name)
}

/// Strip path components from a filename for safe extraction.
pub fn sanitize_filename(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download.jar".into());
    let clean: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') { c } else { '_' })
        .collect();
    if clean.is_empty() {
        "download.jar".into()
    } else {
        clean
    }
}

/// Build the `mods`/`plugins` directory path for a server record.
pub fn content_dir(_server_dir: &Path, server_type: &str) -> Result<&'static str> {
    let t = crate::servers::types::ServerType::parse(server_type)
        .context("unknown server type")?;
    if t.modrinth_facets().is_none() {
        bail!("this server type does not support mods/plugins");
    }
    Ok(t.content_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_sanitized() {
        assert_eq!(sanitize_filename("mod-1.0.jar"), "mod-1.0.jar");
        assert_eq!(sanitize_filename("../evil.jar"), "evil.jar");
        assert_eq!(sanitize_filename("a/b/c.jar"), "c.jar");
        assert_eq!(sanitize_filename("bad\\x.jar"), "bad_x.jar");
        assert_eq!(sanitize_filename(""), "download.jar");
    }

    #[test]
    fn content_dir_per_type() {
        assert_eq!(content_dir(Path::new("/x"), "paper").unwrap(), "plugins");
        assert_eq!(content_dir(Path::new("/x"), "fabric").unwrap(), "mods");
        assert!(content_dir(Path::new("/x"), "vanilla").is_err());
        assert!(content_dir(Path::new("/x"), "bogus").is_err());
    }
}
