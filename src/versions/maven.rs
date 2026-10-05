//! Forge and NeoForge via their Maven repositories.

use anyhow::Result;

use crate::versions::compare;

pub const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
pub const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";

/// Extract every `<version>...</version>` from a maven-metadata.xml body.
pub fn parse_maven_versions(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<version>") {
        rest = &rest[start + "<version>".len()..];
        if let Some(end) = rest.find("</version>") {
            out.push(rest[..end].trim().to_string());
            rest = &rest[end..];
        } else {
            break;
        }
    }
    out
}

/// Forge full version `1.20.1-47.2.0` → (`1.20.1`, `47.2.0`).
pub fn forge_split(full: &str) -> Option<(String, String)> {
    let (mc, fv) = full.split_once('-')?;
    if mc.is_empty() || fv.is_empty() || !mc.chars().next()?.is_ascii_digit() {
        return None;
    }
    Some((mc.to_string(), fv.to_string()))
}

/// NeoForge `21.1.77` → MC `1.21.1`; `20.4.237` → `1.20.4`.
/// Rule: `1.{major}` plus `.{minor}` when minor > 0.
pub fn neoforge_mc(v: &str) -> Option<String> {
    let mut it = v.split('.').map(|p| p.parse::<u64>().ok());
    let major = it.next()??;
    let minor = it.next().flatten().unwrap_or(0);
    if minor == 0 {
        Some(format!("1.{major}"))
    } else {
        Some(format!("1.{major}.{minor}"))
    }
}

/// Deduplicated MC versions from a Forge maven metadata version list,
/// newest first.
pub fn forge_mc_versions(versions: &[String]) -> Vec<String> {
    let mut v: Vec<String> = versions
        .iter()
        .filter_map(|f| forge_split(f).map(|(mc, _)| mc))
        .collect();
    compare::sort_desc(&mut v);
    v
}

/// Loader versions for a given MC version, newest first (Forge versions are
/// dot-numeric so lexical-desc would break on e.g. 47.9 vs 47.10 — sort via
/// a numeric-tuple compare).
pub fn forge_loaders_for(versions: &[String], mc: &str) -> Vec<String> {
    let mut v: Vec<String> = versions
        .iter()
        .filter_map(|f| {
            forge_split(f).and_then(|(m, fv)| (m == mc).then(|| format!("{mc}-{fv}")))
        })
        .collect();
    v.sort_by(|a, b| compare::cmp_versions(&b.replace('-', "."), &a.replace('-', ".")));
    v
}

/// NeoForge installer versions that target `mc`, newest first.
pub fn neoforge_for_mc(versions: &[String], mc: &str) -> Vec<String> {
    let mut v: Vec<String> = versions
        .iter()
        .filter(|n| neoforge_mc(n).as_deref() == Some(mc))
        .cloned()
        .collect();
    v.sort_by(|a, b| compare::cmp_versions(b, a));
    v
}

/// All MC versions covered by a NeoForge metadata list.
pub fn neoforge_mc_versions(versions: &[String]) -> Vec<String> {
    let mut v: Vec<String> = versions
        .iter()
        .filter_map(|n| neoforge_mc(n))
        .collect();
    compare::sort_desc(&mut v);
    v
}

pub fn forge_installer_url(full: &str) -> String {
    format!("{FORGE_MAVEN}/{full}/forge-{full}-installer.jar")
}

pub fn neoforge_installer_url(v: &str) -> String {
    format!("{NEOFORGE_MAVEN}/{v}/neoforge-{v}-installer.jar")
}

pub async fn fetch_maven_versions(http: &reqwest::Client, base: &str) -> Result<Vec<String>> {
    let xml = http
        .get(format!("{base}/maven-metadata.xml"))
        .send()
        .await?
        .text()
        .await?;
    Ok(parse_maven_versions(&xml))
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0"?>
<metadata><versioning><versions>
<version>1.20.1-47.2.0</version>
<version>1.20.1-47.3.0</version>
<version>1.21-51.0.8</version>
<version>1.21.1-52.0.1</version>
<version>47.1.3</version>
</versions></versioning></metadata>"#;

    #[test]
    fn parse_maven_xml() {
        let v = parse_maven_versions(XML);
        assert_eq!(v.len(), 5);
        assert_eq!(v[0], "1.20.1-47.2.0");
        assert!(parse_maven_versions("").is_empty());
        assert!(parse_maven_versions("<version>unclosed").is_empty());
    }

    #[test]
    fn forge_version_split() {
        assert_eq!(
            forge_split("1.20.1-47.2.0"),
            Some(("1.20.1".into(), "47.2.0".into()))
        );
        assert_eq!(forge_split("noversion"), None);
        assert_eq!(forge_split("-47"), None);
        assert_eq!(forge_split("1.20-"), None);
    }

    #[test]
    fn neoforge_mc_mapping() {
        assert_eq!(neoforge_mc("21.1.77"), Some("1.21.1".into()));
        assert_eq!(neoforge_mc("21.0.167"), Some("1.21".into()));
        assert_eq!(neoforge_mc("20.4.237"), Some("1.20.4".into()));
        assert_eq!(neoforge_mc("bad"), None);
        assert_eq!(neoforge_mc("a.b.c"), None);
    }

    #[test]
    fn forge_grouping() {
        let v = parse_maven_versions(XML);
        assert_eq!(forge_mc_versions(&v), vec!["1.21.1", "1.21", "1.20.1"]);
        let loaders = forge_loaders_for(&v, "1.20.1");
        assert_eq!(loaders, vec!["1.20.1-47.3.0", "1.20.1-47.2.0"]);
        assert!(forge_loaders_for(&v, "9.9").is_empty());
    }

    #[test]
    fn neoforge_grouping() {
        let v = vec![
            "21.1.77".to_string(),
            "21.0.5".to_string(),
            "20.4.237".to_string(),
            "21.1.80".to_string(),
        ];
        assert_eq!(neoforge_mc_versions(&v), vec!["1.21.1", "1.21", "1.20.4"]);
        assert_eq!(neoforge_for_mc(&v, "1.21.1"), vec!["21.1.80", "21.1.77"]);
    }

    #[test]
    fn installer_urls() {
        assert_eq!(
            forge_installer_url("1.21-51.0.8"),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.21-51.0.8/forge-1.21-51.0.8-installer.jar"
        );
        assert_eq!(
            neoforge_installer_url("21.1.77"),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.77/neoforge-21.1.77-installer.jar"
        );
    }
}
