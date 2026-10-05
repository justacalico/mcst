//! SQLite access layer. All queries live here so handlers stay thin.

use std::path::Path;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::servers::types::ServerRecord;

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Connect to (and create) the database, then run migrations.
    pub async fn connect(url: &str) -> Result<Self> {
        // Ensure the parent directory exists for file-backed databases.
        if let Some(path) = url.strip_prefix("sqlite:") {
            let path = path.split('?').next().unwrap_or("");
            if !path.is_empty() && path != ":memory:" {
                if let Some(parent) = Path::new(path).parent() {
                    tokio::fs::create_dir_all(parent).await.ok();
                }
            }
        }
        let opts: SqliteConnectOptions = url.parse().context("invalid database URL")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts.create_if_missing(true))
            .await?;
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("PRAGMA foreign_keys=ON").execute(&pool).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // ---------- users ----------

    pub async fn user_count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("c"))
    }

    pub async fn create_user(&self, username: &str, password_hash: &str) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO users (id, username, password_hash) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(username)
            .bind(password_hash)
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn user_by_name(&self, username: &str) -> Result<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, password_hash, created_at FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn user_by_id(&self, id: &str) -> Result<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, password_hash, created_at FROM users WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn set_password(&self, user_id: &str, password_hash: &str) -> Result<()> {
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(password_hash)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---------- sessions ----------

    pub async fn create_session(&self, user_id: &str, token: &str, days: i64) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions (token, user_id, expires_at) \
             VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now', '+' || ? || ' days'))",
        )
        .bind(token)
        .bind(user_id)
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn session_user(&self, token: &str) -> Result<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT u.id, u.username, u.password_hash, u.created_at FROM sessions s \
             JOIN users u ON u.id = s.user_id \
             WHERE s.token = ? AND s.expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete_session(&self, token: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE token = ?")
            .bind(token)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_expired_sessions(&self) -> Result<u64> {
        let r = sqlx::query(
            "DELETE FROM sessions WHERE expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        )
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected())
    }

    pub async fn delete_user_sessions(&self, user_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---------- servers ----------

    pub async fn list_servers(&self) -> Result<Vec<ServerRecord>> {
        Ok(
            sqlx::query_as::<_, ServerRecord>("SELECT * FROM servers ORDER BY created_at")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn get_server(&self, id: &str) -> Result<Option<ServerRecord>> {
        Ok(
            sqlx::query_as::<_, ServerRecord>("SELECT * FROM servers WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn insert_server(&self, s: &ServerRecord) -> Result<()> {
        sqlx::query(
            "INSERT INTO servers (id, name, server_type, mc_version, loader_version, port, \
             memory_mb, min_memory_mb, java_path, jvm_args, dir, jar, auto_start, \
             restart_on_crash, shutdown_timeout_sec, empty_stop_minutes, icon) \
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(&s.id)
        .bind(&s.name)
        .bind(&s.server_type)
        .bind(&s.mc_version)
        .bind(&s.loader_version)
        .bind(s.port)
        .bind(s.memory_mb)
        .bind(s.min_memory_mb)
        .bind(&s.java_path)
        .bind(&s.jvm_args)
        .bind(&s.dir)
        .bind(&s.jar)
        .bind(s.auto_start)
        .bind(s.restart_on_crash)
        .bind(s.shutdown_timeout_sec)
        .bind(s.empty_stop_minutes)
        .bind(&s.icon)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_server(&self, s: &ServerRecord) -> Result<()> {
        sqlx::query(
            "UPDATE servers SET name=?, server_type=?, mc_version=?, loader_version=?, port=?, \
             memory_mb=?, min_memory_mb=?, java_path=?, jvm_args=?, dir=?, jar=?, auto_start=?, \
             restart_on_crash=?, shutdown_timeout_sec=?, empty_stop_minutes=?, icon=?, \
             updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?",
        )
        .bind(&s.name)
        .bind(&s.server_type)
        .bind(&s.mc_version)
        .bind(&s.loader_version)
        .bind(s.port)
        .bind(s.memory_mb)
        .bind(s.min_memory_mb)
        .bind(&s.java_path)
        .bind(&s.jvm_args)
        .bind(&s.dir)
        .bind(&s.jar)
        .bind(s.auto_start)
        .bind(s.restart_on_crash)
        .bind(s.shutdown_timeout_sec)
        .bind(s.empty_stop_minutes)
        .bind(&s.icon)
        .bind(&s.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_server(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM servers WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---------- backups ----------

    pub async fn list_backups(&self, server_id: &str) -> Result<Vec<BackupRow>> {
        Ok(sqlx::query_as::<_, BackupRow>(
            "SELECT * FROM backups WHERE server_id = ? ORDER BY created_at DESC",
        )
        .bind(server_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_backup(&self, id: &str) -> Result<Option<BackupRow>> {
        Ok(
            sqlx::query_as::<_, BackupRow>("SELECT * FROM backups WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn insert_backup(&self, b: &BackupRow) -> Result<()> {
        sqlx::query(
            "INSERT INTO backups (id, server_id, path, size_bytes, note) VALUES (?,?,?,?,?)",
        )
        .bind(&b.id)
        .bind(&b.server_id)
        .bind(&b.path)
        .bind(b.size_bytes)
        .bind(&b.note)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_backup(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM backups WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---------- schedules ----------

    pub async fn list_schedules(&self, server_id: &str) -> Result<Vec<ScheduleRow>> {
        Ok(sqlx::query_as::<_, ScheduleRow>(
            "SELECT * FROM schedules WHERE server_id = ? ORDER BY created_at",
        )
        .bind(server_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn all_schedules(&self) -> Result<Vec<ScheduleRow>> {
        Ok(sqlx::query_as::<_, ScheduleRow>("SELECT * FROM schedules")
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn get_schedule(&self, id: &str) -> Result<Option<ScheduleRow>> {
        Ok(
            sqlx::query_as::<_, ScheduleRow>("SELECT * FROM schedules WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn insert_schedule(&self, s: &ScheduleRow) -> Result<()> {
        sqlx::query(
            "INSERT INTO schedules (id, server_id, name, action, payload, every_minutes, \
             daily_time, enabled, last_run_at) VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(&s.id)
        .bind(&s.server_id)
        .bind(&s.name)
        .bind(&s.action)
        .bind(&s.payload)
        .bind(s.every_minutes)
        .bind(&s.daily_time)
        .bind(s.enabled)
        .bind(&s.last_run_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_schedule(&self, s: &ScheduleRow) -> Result<()> {
        sqlx::query(
            "UPDATE schedules SET name=?, action=?, payload=?, every_minutes=?, daily_time=?, \
             enabled=?, last_run_at=? WHERE id=?",
        )
        .bind(&s.name)
        .bind(&s.action)
        .bind(&s.payload)
        .bind(s.every_minutes)
        .bind(&s.daily_time)
        .bind(s.enabled)
        .bind(&s.last_run_at)
        .bind(&s.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_schedule(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM schedules WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_schedule_run(&self, id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE schedules SET last_run_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ---------- java installs ----------

    pub async fn list_java(&self) -> Result<Vec<JavaRow>> {
        Ok(
            sqlx::query_as::<_, JavaRow>("SELECT * FROM java_installs ORDER BY major")
                .fetch_all(&self.pool)
                .await?,
        )
    }

    pub async fn insert_java(&self, major: i64, path: &str, managed: bool) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO java_installs (id, major, path, managed) VALUES (?,?,?,?)")
            .bind(&id)
            .bind(major)
            .bind(path)
            .bind(managed as i64)
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn delete_java(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM java_installs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---------- settings ----------

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<String, _>("value")))
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_setting(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---------- audit ----------

    pub async fn audit(&self, actor: &str, action: &str, detail: &str) -> Result<()> {
        sqlx::query("INSERT INTO audit (actor, action, detail) VALUES (?,?,?)")
            .bind(actor)
            .bind(action)
            .bind(detail)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn audit_log(&self, limit: i64) -> Result<Vec<AuditRow>> {
        Ok(
            sqlx::query_as::<_, AuditRow>("SELECT * FROM audit ORDER BY id DESC LIMIT ?")
                .bind(limit)
                .fetch_all(&self.pool)
                .await?,
        )
    }
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct BackupRow {
    pub id: String,
    pub server_id: String,
    pub path: String,
    pub size_bytes: i64,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ScheduleRow {
    pub id: String,
    pub server_id: String,
    pub name: String,
    pub action: String,
    pub payload: String,
    pub every_minutes: i64,
    pub daily_time: String,
    pub enabled: i64,
    pub last_run_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct JavaRow {
    pub id: String,
    pub major: i64,
    pub path: String,
    pub managed: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct AuditRow {
    pub id: i64,
    pub ts: String,
    pub actor: String,
    pub action: String,
    pub detail: String,
}
