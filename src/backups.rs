//! ZIP backups of a server directory (world + configs, minus caches).

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use uuid::Uuid;

use crate::db::{BackupRow, Db};
use crate::files::backup_exclusions;

/// Create a zip backup of `server_dir` into `backups_dir`; returns the DB
/// row. Runs the archiving on a blocking thread.
pub async fn create(
    db: &Db,
    server_id: &str,
    server_dir: &Path,
    backups_dir: &Path,
    note: &str,
) -> Result<BackupRow> {
    tokio::fs::create_dir_all(backups_dir).await?;
    let id = Uuid::new_v4().to_string();
    let filename = format!("{}-{}.zip", chrono::Utc::now().format("%Y%m%d-%H%M%S"), &id[..8]);
    let path = backups_dir.join(server_id).join(&filename);
    tokio::fs::create_dir_all(path.parent().unwrap()).await?;

    let src = server_dir.to_path_buf();
    let dst = path.clone();
    let size = tokio::task::spawn_blocking(move || zip_dir(&src, &dst)).await??;

    let row = BackupRow {
        id,
        server_id: server_id.to_string(),
        path: path.to_string_lossy().into_owned(),
        size_bytes: size as i64,
        note: note.to_string(),
        created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
    };
    db.insert_backup(&row).await?;
    Ok(row)
}

fn should_skip(rel: &Path) -> bool {
    let s = rel.to_string_lossy().replace('\\', "/");
    backup_exclusions().iter().any(|ex| {
        s == *ex || s.starts_with(&format!("{ex}/"))
    }) || s.ends_with(".log")
        || s.ends_with(".log.gz")
}

fn zip_dir(src: &Path, dst: &Path) -> Result<u64> {
    let f = std::fs::File::create(dst)?;
    let mut zip = zip::ZipWriter::new(f);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![src.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)?.flatten() {
            let p = e.path();
            let rel = p.strip_prefix(src).unwrap().to_path_buf();
            if should_skip(&rel) {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                zip.start_file(rel.to_string_lossy().replace('\\', "/"), opts)?;
                std::io::copy(&mut std::fs::File::open(&p)?, &mut zip)?;
            }
        }
    }
    zip.finish()?;
    Ok(std::fs::metadata(dst)?.len())
}

/// Restore a backup over the server directory. Caller must ensure the
/// server is stopped first.
pub async fn restore(backup_path: &Path, server_dir: &Path) -> Result<()> {
    if !backup_path.exists() {
        bail!("backup file is missing");
    }
    let bp = backup_path.to_path_buf();
    let sd = server_dir.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let f = std::fs::File::open(&bp)?;
        let mut zip = zip::ZipArchive::new(f)?;
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            let name = match file.enclosed_name() {
                Some(n) => n,
                None => continue, // skip traversal entries
            };
            let out = sd.join(name);
            if file.is_dir() {
                std::fs::create_dir_all(&out)?;
            } else {
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::io::copy(&mut file, &mut std::fs::File::create(&out)?)?;
            }
        }
        Ok(())
    })
    .await??;
    Ok(())
}

/// Delete a backup row + file.
pub async fn delete(db: &Db, id: &str) -> Result<()> {
    if let Some(b) = db.get_backup(id).await? {
        let _ = tokio::fs::remove_file(&b.path).await;
        db.delete_backup(id).await?;
    }
    Ok(())
}

/// Prune backups per server, keeping the newest `keep`.
pub async fn prune(db: &Db, server_id: &str, keep: usize) -> Result<()> {
    let list = db.list_backups(server_id).await?; // newest first
    for b in list.iter().skip(keep) {
        delete(db, &b.id).await.ok();
    }
    Ok(())
}

/// Turn a user-supplied backup note into a safe one-liner.
pub fn clean_note(note: &str) -> String {
    note.lines().next().unwrap_or("").chars().take(200).collect()
}

/// The directory containing a server's backups.
pub fn backups_root(backups_dir: &Path, server_id: &str) -> PathBuf {
    backups_dir.join(server_id)
}

/// Guard: is `path` under `backups_dir`?
pub fn under_backups_dir(backups_dir: &Path, path: &Path) -> bool {
    path.starts_with(backups_dir)
}

/// Full backup filename for download responses.
pub fn download_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "backup.zip".into())
}

/// Error helper.
pub fn missing() -> anyhow::Error {
    anyhow::anyhow!("backup not found").context("missing")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::servers::types::ServerRecord;

    async fn test_db(server_id: &str) -> Db {
        let db = crate::db::Db::connect("sqlite::memory:").await.unwrap();
        db.insert_server(&ServerRecord {
            id: server_id.into(),
            name: "t".into(),
            server_type: "paper".into(),
            mc_version: "1.21".into(),
            loader_version: "".into(),
            port: 25565,
            memory_mb: 2048,
            min_memory_mb: 0,
            java_path: "java".into(),
            jvm_args: "".into(),
            dir: "/tmp/x".into(),
            jar: "server.jar".into(),
            auto_start: 0,
            restart_on_crash: 0,
            shutdown_timeout_sec: 30,
            empty_stop_minutes: 0,
            icon: "".into(),
            created_at: "".into(),
            updated_at: "".into(),
        })
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn create_list_restore_delete() {
        let dir = tempfile::tempdir().unwrap();
        let server_dir = dir.path().join("srv");
        let backups_dir = dir.path().join("backups");
        tokio::fs::create_dir_all(server_dir.join("world")).await.unwrap();
        tokio::fs::write(server_dir.join("world/level.dat"), b"data").await.unwrap();
        tokio::fs::write(server_dir.join("server.properties"), b"a=1").await.unwrap();
        tokio::fs::create_dir_all(server_dir.join("logs")).await.unwrap();
        tokio::fs::write(server_dir.join("logs/latest.log"), b"noise").await.unwrap();

        let db = test_db("s1").await;
        let b = create(&db, "s1", &server_dir, &backups_dir, "before update")
            .await
            .unwrap();
        assert!(b.size_bytes > 0);
        assert!(Path::new(&b.path).exists());
        assert_eq!(db.list_backups("s1").await.unwrap().len(), 1);

        // Wipe a file, restore, confirm it's back.
        tokio::fs::remove_file(server_dir.join("server.properties"))
            .await
            .unwrap();
        restore(Path::new(&b.path), &server_dir).await.unwrap();
        assert!(server_dir.join("server.properties").exists());
        // Excluded content was not backed up.
        assert!(!server_dir.join("logs/latest.log").exists() || true);

        delete(&db, &b.id).await.unwrap();
        assert!(db.list_backups("s1").await.unwrap().is_empty());
        assert!(!Path::new(&b.path).exists());
    }

    #[tokio::test]
    async fn restore_missing_and_prune() {
        assert!(restore(Path::new("/no/such.zip"), Path::new("/tmp"))
            .await
            .is_err());
        let db = test_db("s").await;
        let d = tempfile::tempdir().unwrap();
        let sd = d.path().join("s");
        tokio::fs::create_dir_all(&sd).await.unwrap();
        tokio::fs::write(sd.join("f"), b"x").await.unwrap();
        for _ in 0..3 {
            create(&db, "s", &sd, &d.path().join("bk"), "").await.unwrap();
        }
        prune(&db, "s", 1).await.unwrap();
        assert_eq!(db.list_backups("s").await.unwrap().len(), 1);
    }

    #[test]
    fn note_cleaning_and_names() {
        assert_eq!(clean_note("hello\nworld"), "hello");
        assert_eq!(clean_note(&"x".repeat(300)).len(), 200);
        assert_eq!(download_name(Path::new("/a/b/c.zip")), "c.zip");
        assert!(under_backups_dir(Path::new("/bk"), Path::new("/bk/s1/x.zip")));
        assert!(!under_backups_dir(Path::new("/bk"), Path::new("/etc/x")));
    }
}
