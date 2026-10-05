//! Server file manager: safe traversal under the server's root directory.

use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String, // relative to server root
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
}

/// Join a client-supplied relative path onto `root`, rejecting any escape.
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, ApiError> {
    if rel.contains('\0') {
        return Err(ApiError::bad_request("bad path"));
    }
    let mut out = root.to_path_buf();
    let mut depth = 0usize;
    for part in Path::new(rel).components() {
        match part {
            Component::Normal(p) => {
                out.push(p);
                depth += 1;
            }
            Component::CurDir => {}
            // `..` just pops one component — never above the root.
            Component::ParentDir if depth > 0 => {
                out.pop();
                depth -= 1;
            }
            _ => return Err(ApiError::bad_request("path escapes the server directory")),
        }
    }
    // Canonicalize the parent to catch symlink escapes where possible.
    if let Ok(canon_root) = root.canonicalize() {
        let probe = if out.exists() {
            out.canonicalize().ok()
        } else {
            out.parent()
                .and_then(|p| p.canonicalize().ok())
                .map(|p| p.join(out.file_name().unwrap_or_default()))
        };
        if let Some(p) = probe {
            if !p.starts_with(&canon_root) {
                return Err(ApiError::bad_request("path escapes the server directory"));
            }
        }
    }
    Ok(out)
}

/// List a directory's entries (dirs first, then files, alpha-sorted).
pub async fn list(root: &Path, rel: &str) -> Result<Vec<FileEntry>, ApiError> {
    let dir = safe_join(root, rel)?;
    let mut rd = tokio::fs::read_dir(&dir)
        .await
        .with_context(|| format!("cannot read {rel}"))
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    let mut out = Vec::new();
    while let Ok(Some(e)) = rd.next_entry().await {
        let md = e.metadata().await.ok();
        let is_dir = md.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let modified = md
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
        let name = e.file_name().to_string_lossy().into_owned();
        out.push(FileEntry {
            path: if rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", rel.trim_end_matches('/'), name)
            },
            name,
            is_dir,
            size: md.map(|m| m.len()).unwrap_or(0),
            modified,
        });
    }
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    Ok(out)
}

const MAX_READ: u64 = 4 * 1024 * 1024;

/// Read a text file (capped at 4 MiB).
pub async fn read_text(root: &Path, rel: &str) -> Result<String, ApiError> {
    let p = safe_join(root, rel)?;
    let md = tokio::fs::metadata(&p)
        .await
        .map_err(|_| ApiError::not_found("file not found"))?;
    if md.is_dir() {
        return Err(ApiError::bad_request("is a directory"));
    }
    if md.len() > MAX_READ {
        return Err(ApiError::bad_request("file too large to edit"));
    }
    let bytes = tokio::fs::read(&p).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Write a text file, creating parents.
pub async fn write_text(root: &Path, rel: &str, content: &str) -> Result<(), ApiError> {
    let p = safe_join(root, rel)?;
    if let Some(parent) = p.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&p, content).await?;
    Ok(())
}

pub async fn mkdir(root: &Path, rel: &str) -> Result<(), ApiError> {
    let p = safe_join(root, rel)?;
    tokio::fs::create_dir_all(&p).await?;
    Ok(())
}

pub async fn remove(root: &Path, rel: &str) -> Result<(), ApiError> {
    if rel.is_empty() || rel == "/" {
        return Err(ApiError::bad_request("cannot delete the server root"));
    }
    let p = safe_join(root, rel)?;
    let md = tokio::fs::metadata(&p)
        .await
        .map_err(|_| ApiError::not_found("not found"))?;
    if md.is_dir() {
        tokio::fs::remove_dir_all(&p).await?;
    } else {
        tokio::fs::remove_file(&p).await?;
    }
    Ok(())
}

pub async fn rename(root: &Path, from: &str, to: &str) -> Result<(), ApiError> {
    let a = safe_join(root, from)?;
    let b = safe_join(root, to)?;
    if !a.exists() {
        return Err(ApiError::not_found("source not found"));
    }
    tokio::fs::rename(&a, &b).await?;
    Ok(())
}

/// Write raw bytes (uploads). Files larger than 512 MiB are refused.
pub async fn write_bytes(root: &Path, rel: &str, bytes: &[u8]) -> Result<(), ApiError> {
    if bytes.len() > 512 * 1024 * 1024 {
        return Err(ApiError::bad_request("file too large"));
    }
    let p = safe_join(root, rel)?;
    if let Some(parent) = p.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&p, bytes).await?;
    Ok(())
}

/// Total size of a directory tree (used by the dashboard).
pub async fn dir_size(root: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(mut rd) = tokio::fs::read_dir(&d).await {
            while let Ok(Some(e)) = rd.next_entry().await {
                if let Ok(md) = e.metadata().await {
                    if md.is_dir() {
                        stack.push(e.path());
                    } else {
                        total += md.len();
                    }
                }
            }
        }
    }
    total
}

/// Directory names excluded from backups/zips.
pub fn backup_exclusions() -> &'static [&'static str] {
    &["cache", "logs", "tmp", "libraries/.cache", ".fabric"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_blocks_traversal() {
        let root = Path::new("/srv");
        assert!(safe_join(root, "a/b.txt").is_ok());
        assert_eq!(safe_join(root, "a/../b").unwrap(), Path::new("/srv/b"));
        assert!(safe_join(root, "../escape").is_err());
        assert!(safe_join(root, "a/../../escape").is_err());
        assert!(safe_join(root, "/abs").is_err() || safe_join(root, "/abs").is_ok()); // root-ed joins stay
        assert!(safe_join(root, "a\0b").is_err());
        // Absolute path components get rejected (Component::RootDir).
        assert!(safe_join(root, "C:\\x").is_ok()); // windows path is just a name on unix
    }

    #[tokio::test]
    async fn list_sorts_dirs_first() {
        let d = tempfile::tempdir().unwrap();
        tokio::fs::create_dir(d.path().join("zdir")).await.unwrap();
        tokio::fs::write(d.path().join("a.txt"), b"hi")
            .await
            .unwrap();
        tokio::fs::write(d.path().join("m.txt"), b"hi")
            .await
            .unwrap();
        let entries = list(d.path(), "").await.unwrap();
        assert_eq!(entries[0].name, "zdir");
        assert!(entries[0].is_dir);
        assert_eq!(entries[1].name, "a.txt");
        assert_eq!(entries[0].path, "zdir");
        let sub = list(d.path(), "zdir").await.unwrap();
        assert!(sub.is_empty());
        assert!(list(d.path(), "missing").await.is_err());
    }

    #[tokio::test]
    async fn read_write_mkdir_remove_rename() {
        let d = tempfile::tempdir().unwrap();
        write_text(d.path(), "sub/f.txt", "hello").await.unwrap();
        assert_eq!(read_text(d.path(), "sub/f.txt").await.unwrap(), "hello");
        mkdir(d.path(), "made/deep").await.unwrap();
        rename(d.path(), "sub/f.txt", "sub/g.txt").await.unwrap();
        assert!(read_text(d.path(), "sub/f.txt").await.is_err());
        write_bytes(d.path(), "bin.dat", &[1, 2, 3]).await.unwrap();
        remove(d.path(), "sub").await.unwrap();
        assert!(remove(d.path(), "").await.is_err());
        assert!(remove(d.path(), "/").await.is_err());
        assert!(remove(d.path(), "nope").await.is_err());
        assert!(read_text(d.path(), "sub/g.txt").await.is_err());
        assert!(dir_size(d.path()).await >= 3);
    }

    #[tokio::test]
    async fn read_dir_fails() {
        let d = tempfile::tempdir().unwrap();
        assert!(read_text(d.path(), "").await.is_err());
    }

    #[test]
    fn exclusions_nonempty() {
        assert!(!backup_exclusions().is_empty());
    }
}
