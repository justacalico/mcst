//! Paper and Purpur — nearly identical v2 JSON APIs.

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::versions::compare;

const PAPER_API: &str = "https://api.papermc.io/v2/projects/paper";
const PURPUR_API: &str = "https://api.purpurmc.org/v2/purpur";

#[derive(Debug, Deserialize)]
struct Project {
    versions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct VersionBuilds {
    builds: Vec<i64>,
}

/// Parse the project version list (API returns oldest-first).
pub fn parse_versions(json: &serde_json::Value) -> Vec<String> {
    let mut v: Vec<String> = json
        .get("versions")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    compare::sort_desc(&mut v);
    v
}

fn api_base(project: &str) -> &'static str {
    match project {
        "purpur" => PURPUR_API,
        _ => PAPER_API,
    }
}

pub async fn list(http: &reqwest::Client, project: &str) -> Result<Vec<String>> {
    let p: Project = http.get(api_base(project)).send().await?.json().await?;
    let mut v = p.versions;
    compare::sort_desc(&mut v);
    Ok(v)
}

/// Latest build number for a version.
pub async fn latest_build(http: &reqwest::Client, project: &str, mc: &str) -> Result<i64> {
    let url = format!("{}/versions/{}", api_base(project), mc);
    let b: VersionBuilds = http.get(&url).send().await?.json().await?;
    b.builds
        .iter()
        .max()
        .copied()
        .with_context(|| format!("no {project} builds for {mc}"))
}

/// Direct download URL for a build.
pub fn download_url(project: &str, mc: &str, build: i64) -> String {
    match project {
        "purpur" => format!("{PURPUR_API}/{mc}/{build}/download"),
        _ => format!("{PAPER_API}/versions/{mc}/builds/{build}/downloads/paper-{mc}-{build}.jar"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_versions_orders_desc() {
        let j = serde_json::json!({"versions": ["1.19", "1.21", "1.20.4"]});
        assert_eq!(parse_versions(&j), vec!["1.21", "1.20.4", "1.19"]);
        assert!(parse_versions(&serde_json::json!({})).is_empty());
        assert!(parse_versions(&serde_json::json!({"versions": "x"})).is_empty());
    }

    #[test]
    fn download_urls() {
        assert!(download_url("paper", "1.21", 130)
            .ends_with("/projects/paper/versions/1.21/builds/130/downloads/paper-1.21-130.jar"));
        assert_eq!(
            download_url("purpur", "1.21", "2444".parse().unwrap()),
            "https://api.purpurmc.org/v2/purpur/1.21/2444/download"
        );
        assert_eq!(api_base("paper"), PAPER_API);
        assert_eq!(api_base("anything"), PAPER_API);
    }
}
