//! Java discovery and managed installs (Adoptium Temurin downloads).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct JavaInstall {
    /// Registry row id — set for managed installs so the UI can delete them.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    pub path: String,
    pub major: u32,
    pub version: String,
    pub managed: bool,
}

static RE_VERSION: Lazy<Regex> = Lazy::new(|| Regex::new(r#"version "([0-9][0-9._]*)"#).unwrap());

/// Parse `java -version` stderr text → (major, full version).
/// Handles both `1.8.0_422` and `17.0.12`/`21.0.3` schemes.
pub fn parse_java_version(text: &str) -> Option<(u32, String)> {
    let c = RE_VERSION.captures(text)?;
    let full = c[1].to_string();
    let mut it = full.split(['.', '_']);
    let first: u32 = it.next()?.parse().ok()?;
    let major = if first == 1 {
        it.next()?.parse().ok()? // "1.8.0_422" → 8
    } else {
        first
    };
    Some((major, full))
}

/// Run `java -version` on a candidate binary.
async fn probe(bin: &Path) -> Option<JavaInstall> {
    let out = tokio::process::Command::new(bin)
        .arg("-version")
        .output()
        .await
        .ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let (major, version) = parse_java_version(&text)?;
    Some(JavaInstall {
        id: String::new(),
        path: bin.to_string_lossy().into_owned(),
        major,
        version,
        managed: false,
    })
}

/// Candidate java binary paths to probe.
fn candidates(managed_dir: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(p) = which::which("java") {
        v.push(p);
    }
    // Managed runtimes installed by mcst.
    if let Ok(rd) = std::fs::read_dir(managed_dir) {
        for e in rd.flatten() {
            let p = e.path();
            for sub in ["bin/java", "bin/java.exe", "Contents/Home/bin/java"] {
                let cand = p.join(sub);
                if cand.exists() {
                    v.push(cand);
                }
            }
        }
    }
    // Common system locations.
    for base in [
        "/usr/lib/jvm",
        "/usr/java",
        "C:\\Program Files\\Java",
        "C:\\Program Files\\Eclipse Adoptium",
    ] {
        if let Ok(rd) = std::fs::read_dir(base) {
            for e in rd.flatten() {
                for sub in ["bin/java", "bin/java.exe", "Contents/Home/bin/java"] {
                    let cand = e.path().join(sub);
                    if cand.exists() {
                        v.push(cand);
                    }
                }
            }
        }
    }
    v
}

/// Probe every candidate, deduplicating by canonical path.
pub async fn detect(managed_dir: &Path) -> Vec<JavaInstall> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for c in candidates(managed_dir) {
        let key = c.canonicalize().unwrap_or_else(|_| c.clone());
        if !seen.insert(key) {
            continue;
        }
        if let Some(j) = probe(&c).await {
            out.push(j);
        }
    }
    out.sort_by_key(|j| j.major);
    out
}

/// Adoptium download URL for a JRE feature release.
pub fn adoptium_url(major: u32) -> String {
    let (os, arch, ext) = platform();
    let _ = ext;
    format!(
        "https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jre/hotspot/normal/eclipse"
    )
}

fn platform() -> (&'static str, &'static str, &'static str) {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    let ext = if cfg!(target_os = "windows") {
        "zip"
    } else {
        "tar.gz"
    };
    (os, arch, ext)
}

/// Download and unpack a Temurin JRE into `dest_dir`; returns the java
/// binary path.
pub async fn install(http: &reqwest::Client, major: u32, dest_dir: &Path) -> Result<String> {
    tokio::fs::create_dir_all(dest_dir).await?;
    let url = adoptium_url(major);
    let bytes = http
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await
        .context("adoptium download failed")?;

    let dest = dest_dir.to_path_buf();
    let is_zip = cfg!(target_os = "windows");
    tokio::task::spawn_blocking(move || unpack_runtime(&bytes, &dest, is_zip)).await??;

    // Locate the java binary inside the unpacked tree.
    find_java_bin(dest_dir)
        .await
        .map(|p| p.to_string_lossy().into_owned())
        .context("java binary not found after unpack")
}

fn unpack_runtime(bytes: &[u8], dest: &Path, is_zip: bool) -> Result<()> {
    if is_zip {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        z.extract(dest)?;
    } else {
        let gz = flate2::read::GzDecoder::new(bytes);
        let mut ar = tar::Archive::new(gz);
        ar.unpack(dest)?;
    }
    Ok(())
}

/// Breadth-first search for `bin/java` under a directory.
pub async fn find_java_bin(root: &Path) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let mut rd = tokio::fs::read_dir(&d).await.ok()?;
        while let Ok(Some(e)) = rd.next_entry().await {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "bin") {
                    for name in ["java", "java.exe"] {
                        let c = p.join(name);
                        if c.exists() {
                            return Some(c);
                        }
                    }
                }
                stack.push(p);
            }
        }
    }
    None
}

/// Best installed runtime for a required major version: exact match, else
/// the closest higher major, else the highest available.
pub fn pick(installs: &[JavaInstall], required: u32) -> Option<JavaInstall> {
    if installs.is_empty() {
        return None;
    }
    installs
        .iter()
        .find(|j| j.major == required)
        .or_else(|| {
            installs
                .iter()
                .filter(|j| j.major > required)
                .min_by_key(|j| j.major)
        })
        .or_else(|| installs.iter().max_by_key(|j| j.major))
        .cloned()
}

/// The required Java major for a MC version (delegates to mojang mapping).
pub fn required_major(mc_version: &str) -> u32 {
    crate::versions::mojang::required_java(mc_version)
}

/// Readable error when no usable Java exists.
pub fn missing_java(required: u32) -> anyhow::Error {
    anyhow::anyhow!(
        "no Java {required}+ runtime found — install one from Settings → Java, \
         or install Java on the host"
    )
}

#[allow(dead_code)]
fn _assert_send<T: Send>(_: &T) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_versions() {
        let (m, v) = parse_java_version(r#"openjdk version "17.0.12" 2024-07-16"#).unwrap();
        assert_eq!((m, v.as_str()), (17, "17.0.12"));
        let (m, _) = parse_java_version(r#"java version "1.8.0_422""#).unwrap();
        assert_eq!(m, 8);
        let (m, _) = parse_java_version(r#"openjdk version "21.0.3" "#).unwrap();
        assert_eq!(m, 21);
        assert!(parse_java_version("garbage").is_none());
        assert!(parse_java_version(r#"version "x.y""#).is_none());
    }

    #[test]
    fn adoptium_url_shape() {
        let u = adoptium_url(21);
        assert!(u.contains("/latest/21/ga/"));
        assert!(u.contains("/jre/hotspot/normal/eclipse"));
    }

    #[test]
    fn pick_best_runtime() {
        let j = |major: u32| JavaInstall {
            id: String::new(),
            path: format!("/j/{major}"),
            major,
            version: major.to_string(),
            managed: false,
        };
        let installs = vec![j(8), j(17), j(21)];
        assert_eq!(pick(&installs, 8).unwrap().major, 8);
        assert_eq!(pick(&installs, 16).unwrap().major, 17); // next higher
        assert_eq!(pick(&installs, 21).unwrap().major, 21);
        assert_eq!(pick(&installs, 99).unwrap().major, 21); // highest fallback
        assert!(pick(&[], 17).is_none());
    }

    #[test]
    fn required_major_mapping() {
        assert_eq!(required_major("1.16.5"), 8);
        assert_eq!(required_major("1.20.4"), 17);
        assert_eq!(required_major("1.21"), 21);
    }

    #[tokio::test]
    async fn find_java_bin_walks_tree() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("jdk-21/bin");
        tokio::fs::create_dir_all(&bin).await.unwrap();
        tokio::fs::write(bin.join("java"), b"#!/bin/sh\n")
            .await
            .unwrap();
        assert!(find_java_bin(d.path()).await.is_some());
        let empty = tempfile::tempdir().unwrap();
        assert!(find_java_bin(empty.path()).await.is_none());
    }

    #[tokio::test]
    async fn detect_finds_something_on_this_host() {
        // The dev machine has java on PATH; detection should find ≥1 entry.
        let d = tempfile::tempdir().unwrap();
        let found = detect(d.path()).await;
        if which::which("java").is_ok() {
            assert!(!found.is_empty());
            assert!(found.iter().all(|j| j.major > 0));
        }
    }
}
