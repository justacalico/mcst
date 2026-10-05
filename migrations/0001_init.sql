-- Initial schema: account, sessions, servers, backups, schedules,
-- settings, java installs, audit log.

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS sessions (
    token      TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    expires_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expiry ON sessions(expires_at);

CREATE TABLE IF NOT EXISTS servers (
    id                   TEXT PRIMARY KEY,
    name                 TEXT NOT NULL,
    server_type          TEXT NOT NULL,           -- vanilla|paper|purpur|fabric|forge|neoforge|custom
    mc_version           TEXT NOT NULL,
    loader_version       TEXT NOT NULL DEFAULT '',
    port                 INTEGER NOT NULL,
    memory_mb            INTEGER NOT NULL DEFAULT 2048,
    min_memory_mb        INTEGER NOT NULL DEFAULT 0,
    java_path            TEXT NOT NULL DEFAULT 'java',
    jvm_args             TEXT NOT NULL DEFAULT '',
    dir                  TEXT NOT NULL,
    jar                  TEXT NOT NULL DEFAULT 'server.jar',
    auto_start           INTEGER NOT NULL DEFAULT 0,
    restart_on_crash     INTEGER NOT NULL DEFAULT 0,
    shutdown_timeout_sec INTEGER NOT NULL DEFAULT 30,
    empty_stop_minutes   INTEGER NOT NULL DEFAULT 0, -- 0 = never auto-stop when empty
    icon                 TEXT NOT NULL DEFAULT '',
    created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_servers_name ON servers(name);

CREATE TABLE IF NOT EXISTS backups (
    id         TEXT PRIMARY KEY,
    server_id  TEXT NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    note       TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_backups_server ON backups(server_id);

CREATE TABLE IF NOT EXISTS schedules (
    id            TEXT PRIMARY KEY,
    server_id     TEXT NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    action        TEXT NOT NULL,          -- command|start|stop|restart|backup
    payload       TEXT NOT NULL DEFAULT '', -- command text or backup note
    every_minutes INTEGER NOT NULL DEFAULT 0,
    daily_time    TEXT NOT NULL DEFAULT '', -- "HH:MM" local time
    enabled       INTEGER NOT NULL DEFAULT 1,
    last_run_at   TEXT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX IF NOT EXISTS idx_schedules_server ON schedules(server_id);

CREATE TABLE IF NOT EXISTS java_installs (
    id         TEXT PRIMARY KEY,
    major      INTEGER NOT NULL,
    path       TEXT NOT NULL,
    managed    INTEGER NOT NULL DEFAULT 0, -- downloaded by mcst via Adoptium
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    ts      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    actor   TEXT NOT NULL DEFAULT '',
    action  TEXT NOT NULL,
    detail  TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_audit_ts ON audit(ts);
