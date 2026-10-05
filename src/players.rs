//! Player management: ops/whitelist/bans JSON files + live commands.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::servers::ServerManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerEntry {
    pub uuid: String,
    pub name: String,
    #[serde(default)]
    pub level: Option<i64>,
    #[serde(default, rename = "bypassesPlayerLimit")]
    pub bypasses: Option<bool>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub expires: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
}

/// Which list file we're editing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListKind {
    Ops,
    Whitelist,
    Bans,
}

impl ListKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "ops" => Self::Ops,
            "whitelist" => Self::Whitelist,
            "bans" => Self::Bans,
            _ => return None,
        })
    }
    pub fn file(self) -> &'static str {
        match self {
            Self::Ops => "ops.json",
            Self::Whitelist => "whitelist.json",
            Self::Bans => "banned-players.json",
        }
    }
    /// In-game command verbs.
    pub fn add_cmd(self) -> &'static str {
        match self {
            Self::Ops => "op",
            Self::Whitelist => "whitelist add",
            Self::Bans => "ban",
        }
    }
    pub fn remove_cmd(self) -> &'static str {
        match self {
            Self::Ops => "deop",
            Self::Whitelist => "whitelist remove",
            Self::Bans => "pardon",
        }
    }
}

async fn read_list(server_dir: &Path, kind: ListKind) -> Result<Vec<PlayerEntry>> {
    let p = server_dir.join(kind.file());
    match tokio::fs::read_to_string(&p).await {
        Ok(s) => Ok(serde_json::from_str(&s).unwrap_or_default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}

/// Mojang API lookup: name → (uuid, canonical name).
pub async fn lookup(http: &reqwest::Client, name: &str) -> Result<(String, String)> {
    #[derive(Deserialize)]
    struct Prof {
        id: String,
        name: String,
    }
    let url = format!("https://api.mojang.com/users/profiles/minecraft/{name}");
    let res = http.get(&url).send().await?;
    if res.status() == reqwest::StatusCode::NO_CONTENT || res.status() == reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!("player '{name}' not found");
    }
    let p: Prof = res.json().await?;
    // Mojang returns the dashed-less UUID; add dashes.
    Ok((dash_uuid(&p.id), p.name))
}

/// Insert dashes into a 32-char UUID.
pub fn dash_uuid(raw: &str) -> String {
    if raw.len() == 32 && !raw.contains('-') {
        format!(
            "{}-{}-{}-{}-{}",
            &raw[..8],
            &raw[8..12],
            &raw[12..16],
            &raw[16..20],
            &raw[20..]
        )
    } else {
        raw.to_string()
    }
}

/// Strip dashes (for lookups that need the compact form).
pub fn undash_uuid(u: &str) -> String {
    u.replace('-', "")
}

/// Read one of the three lists.
pub async fn list(server_dir: &Path, kind: ListKind) -> Result<Vec<PlayerEntry>> {
    read_list(server_dir, kind).await
}

/// Add a player to a list file (and push the in-game command when the
/// server is running so it applies immediately).
pub async fn add(
    mgr: &ServerManager,
    server_dir: &Path,
    server_id: &str,
    kind: ListKind,
    name: &str,
    http: &reqwest::Client,
) -> Result<Vec<PlayerEntry>, ApiError> {
    if !valid_name(name) {
        return Err(ApiError::bad_request("invalid player name"));
    }
    let mut entries = read_list(server_dir, kind)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if entries.iter().any(|e| e.name.eq_ignore_ascii_case(name)) {
        return Ok(entries);
    }
    let (uuid, canon) = match lookup(http, name).await {
        Ok(v) => v,
        Err(_) => (offline_uuid(name), name.to_string()),
    };
    entries.push(PlayerEntry {
        uuid,
        name: canon.clone(),
        level: (kind == ListKind::Ops).then_some(4),
        bypasses: None,
        reason: (kind == ListKind::Bans).then(|| "Banned by mcst".to_string()),
        expires: None,
        source: (kind == ListKind::Bans).then(|| "mcst".to_string()),
        created: (kind == ListKind::Bans).then(|| chrono::Utc::now().to_rfc3339()),
    });
    write_list(server_dir, kind, &entries).await?;
    let _ = mgr
        .send_command(server_id, &format!("{} {}", kind.add_cmd(), canon))
        .await;
    Ok(entries)
}

pub async fn remove(
    mgr: &ServerManager,
    server_dir: &Path,
    server_id: &str,
    kind: ListKind,
    name: &str,
) -> Result<Vec<PlayerEntry>, ApiError> {
    let mut entries = read_list(server_dir, kind)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    entries.retain(|e| !e.name.eq_ignore_ascii_case(name));
    write_list(server_dir, kind, &entries).await?;
    let _ = mgr
        .send_command(server_id, &format!("{} {}", kind.remove_cmd(), name))
        .await;
    Ok(entries)
}

async fn write_list(server_dir: &Path, kind: ListKind, entries: &[PlayerEntry]) -> Result<(), ApiError> {
    let p = server_dir.join(kind.file());
    tokio::fs::write(&p, serde_json::to_string_pretty(entries)?).await?;
    Ok(())
}

/// A plausible offline-mode UUID (deterministic per name).
pub fn offline_uuid(name: &str) -> String {
    use sha2::Digest;
    let h = sha2::Sha256::digest(format!("OfflinePlayer:{}", name.to_lowercase()).as_bytes());
    let mut hexs = hex::encode(&h[..16]);
    // Set version 3 (name-based MD5-style) bits.
    hexs.replace_range(12..13, "3");
    hexs.replace_range(16..17, "8");
    dash_uuid(&hexs[..32])
}

/// Minecraft name rules: 3-16 chars, alphanumeric + underscore.
pub fn valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_roundtrip() {
        assert_eq!(ListKind::parse("ops"), Some(ListKind::Ops));
        assert_eq!(ListKind::parse("whitelist"), Some(ListKind::Whitelist));
        assert_eq!(ListKind::parse("bans"), Some(ListKind::Bans));
        assert_eq!(ListKind::parse("x"), None);
        assert_eq!(ListKind::Ops.file(), "ops.json");
        assert_eq!(ListKind::Whitelist.file(), "whitelist.json");
        assert_eq!(ListKind::Bans.file(), "banned-players.json");
        assert_eq!(ListKind::Ops.add_cmd(), "op");
        assert_eq!(ListKind::Ops.remove_cmd(), "deop");
        assert_eq!(ListKind::Bans.add_cmd(), "ban");
        assert_eq!(ListKind::Bans.remove_cmd(), "pardon");
        assert_eq!(ListKind::Whitelist.add_cmd(), "whitelist add");
    }

    #[test]
    fn uuid_helpers() {
        let raw = "069a79f444e94726a5befca90e38aaf5";
        assert_eq!(
            dash_uuid(raw),
            "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        );
        assert_eq!(dash_uuid("already-dashed-x"), "already-dashed-x");
        assert_eq!(undash_uuid("a-b"), "ab");
        let off = offline_uuid("Steve");
        assert_eq!(off.len(), 36);
        assert_eq!(offline_uuid("Steve"), offline_uuid("steve")); // case-insensitive
    }

    #[test]
    fn name_rules() {
        assert!(valid_name("Steve"));
        assert!(valid_name("x_y_z"));
        assert!(!valid_name("ab")); // too short
        assert!(!valid_name("waytoolongplayername1"));
        assert!(!valid_name("bad name"));
        assert!(!valid_name(""));
    }

    #[tokio::test]
    async fn list_io() {
        let d = tempfile::tempdir().unwrap();
        let e = vec![PlayerEntry {
            uuid: "u".into(),
            name: "Steve".into(),
            level: Some(4),
            bypasses: Some(false),
            reason: None,
            expires: None,
            source: None,
            created: None,
        }];
        write_list(d.path(), ListKind::Ops, &e).await.unwrap();
        let back = list(d.path(), ListKind::Ops).await.unwrap();
        assert_eq!(back[0].name, "Steve");
        assert!(list(d.path(), ListKind::Bans).await.unwrap().is_empty());
    }
}
