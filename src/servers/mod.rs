//! Server lifecycle management: spawn, supervise, console, stats.

pub mod command;
pub mod logparse;
pub mod props;
pub mod types;

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::{broadcast, Mutex, RwLock};
use uuid::Uuid;

use crate::config::Config;
use crate::db::Db;
use crate::servers::types::{PanelEvent, ServerDto, ServerRecord, ServerStatus};

const LOG_HISTORY: usize = 2000;
const MAX_CRASH_RESTARTS: u32 = 5;
const CRASH_RESTART_DELAY: Duration = Duration::from_secs(5);

/// Player presence tracked for one server.
#[derive(Debug, Default)]
pub struct PlayerState {
    pub names: BTreeSet<String>,
    pub max: i64,
}

/// Live runtime state for one server.
pub struct Runtime {
    pub record: RwLock<ServerRecord>,
    status: RwLock<ServerStatus>,
    history: RwLock<VecDeque<String>>,
    log_tx: broadcast::Sender<String>,
    stdin: Mutex<Option<ChildStdin>>,
    child_pid: AtomicU32,
    started_at: RwLock<Option<Instant>>,
    players: RwLock<PlayerState>,
    last_exit: RwLock<Option<i32>>,
    cpu_bits: AtomicU64,
    mem_bytes: AtomicU64,
    last_nonempty: RwLock<Instant>,
    crash_restarts: AtomicU32,
    /// Set when a stop was requested by the user — suppresses crash restart.
    manual_stop: std::sync::atomic::AtomicBool,
}

impl Runtime {
    fn new(record: ServerRecord) -> Self {
        let (log_tx, _) = broadcast::channel(512);
        Self {
            record: RwLock::new(record),
            status: RwLock::new(ServerStatus::Stopped),
            history: RwLock::new(VecDeque::with_capacity(LOG_HISTORY)),
            log_tx,
            stdin: Mutex::new(None),
            child_pid: AtomicU32::new(0),
            started_at: RwLock::new(None),
            players: RwLock::new(PlayerState::default()),
            last_exit: RwLock::new(None),
            cpu_bits: AtomicU64::new(0),
            mem_bytes: AtomicU64::new(0),
            last_nonempty: RwLock::new(Instant::now()),
            crash_restarts: AtomicU32::new(0),
            manual_stop: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub async fn status(&self) -> ServerStatus {
        *self.status.read().await
    }

    /// Set the status without emitting an event (install flow).
    pub async fn force_status(&self, status: ServerStatus) {
        *self.status.write().await = status;
    }

    pub async fn push_log(&self, line: String) {
        {
            let mut h = self.history.write().await;
            if h.len() >= LOG_HISTORY {
                h.pop_front();
            }
            h.push_back(line.clone());
        }
        let _ = self.log_tx.send(line);
    }

    /// History snapshot + a live subscription.
    pub async fn subscribe(&self) -> (Vec<String>, broadcast::Receiver<String>) {
        (
            self.history.read().await.iter().cloned().collect(),
            self.log_tx.subscribe(),
        )
    }

    pub fn set_stats(&self, cpu: f64, mem: u64) {
        self.cpu_bits.store(cpu.to_bits(), Ordering::Relaxed);
        self.mem_bytes.store(mem, Ordering::Relaxed);
    }

    pub fn stats(&self) -> (f64, u64) {
        (
            f64::from_bits(self.cpu_bits.load(Ordering::Relaxed)),
            self.mem_bytes.load(Ordering::Relaxed),
        )
    }
}

/// Central lifecycle manager shared across the app.
#[derive(Clone)]
pub struct ServerManager {
    inner: Arc<Inner>,
}

struct Inner {
    db: Db,
    cfg: Config,
    runtimes: Mutex<HashMap<String, Arc<Runtime>>>,
    events: broadcast::Sender<PanelEvent>,
    sysinfo: Mutex<sysinfo::System>,
    /// Crash-restart requests — a channel breaks the recursive async cycle
    /// between process exit and re-spawn.
    restart_tx: tokio::sync::mpsc::UnboundedSender<String>,
    restart_rx: Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<String>>>,
}

impl ServerManager {
    pub fn new(db: Db, cfg: Config) -> Self {
        let (events, _) = broadcast::channel(256);
        let (restart_tx, restart_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            inner: Arc::new(Inner {
                db,
                cfg,
                runtimes: Mutex::new(HashMap::new()),
                events,
                sysinfo: Mutex::new(sysinfo::System::new()),
                restart_tx,
                restart_rx: Mutex::new(Some(restart_rx)),
            }),
        }
    }

    /// Start the crash-restart supervisor. Called once at boot.
    pub fn start_supervisor(&self) {
        let rx = {
            let Ok(mut guard) = self.inner.restart_rx.try_lock() else {
                return;
            };
            guard.take()
        };
        let Some(mut rx) = rx else { return };
        let mgr = self.clone();
        tokio::spawn(async move {
            while let Some(id) = rx.recv().await {
                tokio::time::sleep(CRASH_RESTART_DELAY).await;
                if let Err(e) = mgr.start_inner(&id, false).await {
                    tracing::warn!(server = %id, "crash restart failed: {e}");
                }
            }
        });
    }

    pub fn events(&self) -> broadcast::Receiver<PanelEvent> {
        self.inner.events.subscribe()
    }

    /// Backups directory from the manager's config.
    pub fn backups_dir(&self) -> PathBuf {
        self.inner.cfg.backups_dir()
    }

    /// Panel config access for helpers.
    pub fn config(&self) -> &Config {
        &self.inner.cfg
    }

    async fn emit(&self, ev: PanelEvent) {
        let _ = self.inner.events.send(ev);
    }

    /// Load or create the runtime for a server id.
    pub async fn runtime(&self, id: &str) -> Result<Arc<Runtime>> {
        {
            let rts = self.inner.runtimes.lock().await;
            if let Some(rt) = rts.get(id) {
                return Ok(rt.clone());
            }
        }
        let rec = self
            .inner
            .db
            .get_server(id)
            .await?
            .ok_or_else(|| anyhow!("server not found"))?;
        let rt = Arc::new(Runtime::new(rec));
        self.inner
            .runtimes
            .lock()
            .await
            .insert(id.to_string(), rt.clone());
        Ok(rt)
    }

    /// Drop the cached runtime (after deletion).
    pub async fn evict(&self, id: &str) {
        self.inner.runtimes.lock().await.remove(id);
    }

    async fn set_status(&self, rt: &Arc<Runtime>, status: ServerStatus) {
        *rt.status.write().await = status;
        let id = rt.record.read().await.id.clone();
        self.emit(PanelEvent::status(&id, status)).await;
    }

    /// Full DTO for one server.
    pub async fn dto(&self, id: &str) -> Result<ServerDto> {
        let rt = self.runtime(id).await?;
        Ok(self.dto_of(&rt).await)
    }

    pub async fn dto_of(&self, rt: &Arc<Runtime>) -> ServerDto {
        let record = rt.record.read().await.clone();
        let status = *rt.status.read().await;
        let players = rt.players.read().await;
        let (cpu, mem) = rt.stats();
        let uptime = rt
            .started_at
            .read()
            .await
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        ServerDto {
            record,
            status,
            players_online: players.names.len() as i64,
            players_max: players.max,
            player_names: players.names.iter().cloned().collect(),
            cpu_percent: cpu,
            mem_bytes: mem,
            uptime_sec: uptime,
            last_exit_code: *rt.last_exit.read().await,
        }
    }

    /// DTOs for every server (dashboard listing).
    pub async fn list_dtos(&self) -> Result<Vec<ServerDto>> {
        let records = self.inner.db.list_servers().await?;
        let mut out = Vec::with_capacity(records.len());
        for rec in records {
            let rt = self.runtime(&rec.id).await?;
            *rt.record.write().await = rec;
            out.push(self.dto_of(&rt).await);
        }
        Ok(out)
    }

    /// Start a server. No-op error if already active.
    pub async fn start(&self, id: &str) -> Result<()> {
        self.start_inner(id, true).await
    }

    async fn start_inner(&self, id: &str, manual: bool) -> Result<()> {
        let rt = self.runtime(id).await?;
        if rt.status().await.is_active() {
            bail!("server is already running");
        }
        let rec = rt.record.read().await.clone();
        let dir = PathBuf::from(&rec.dir);
        if !dir.join(&rec.jar).exists()
            && crate::servers::command::find_args_file(&dir)
                .await
                .is_none()
        {
            bail!("server jar not found — the install may be incomplete");
        }
        self.set_status(&rt, ServerStatus::Starting).await;
        if manual {
            rt.manual_stop.store(false, Ordering::SeqCst);
            rt.crash_restarts.store(0, Ordering::SeqCst);
        }
        self.spawn_process(rt.clone(), rec).await
    }

    async fn spawn_process(&self, rt: Arc<Runtime>, rec: ServerRecord) -> Result<()> {
        let dir = PathBuf::from(&rec.dir);
        tokio::fs::create_dir_all(&dir).await.ok();
        let (program, args) = command::build_command(&dir, &rec).await;
        rt.push_log(format!("[mcst] starting: {program} {}", args.join(" ")))
            .await;

        let mut cmd = tokio::process::Command::new(&program);
        cmd.args(&args)
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // MCP/forge servers don't need a controlling terminal; plain pipes
        // give the most compatible stdin/stdout behavior on all platforms.
        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to launch '{program}' — is Java installed?"))?;

        if let Some(pid) = child.id() {
            rt.child_pid.store(pid, Ordering::SeqCst);
        }
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to capture server stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to capture server stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("failed to capture server stderr"))?;
        *rt.stdin.lock().await = Some(stdin);
        *rt.started_at.write().await = Some(Instant::now());
        rt.players.write().await.names.clear();

        // Pump both output streams into the log.
        let rt_out = rt.clone();
        let mut out_lines = BufReader::new(stdout).lines();
        let mut err_lines = BufReader::new(stderr).lines();
        tokio::spawn(async move {
            loop {
                let line = tokio::select! {
                    l = out_lines.next_line() => l,
                    l = err_lines.next_line() => l,
                };
                match line {
                    Ok(Some(l)) => {
                        rt_out.handle_log_line(&l).await;
                        rt_out.push_log(l).await;
                    }
                    Ok(None) => {
                        // One stream closed; wait on the other one.
                        let rest = if out_lines.next_line().await.ok().flatten().is_none() {
                            err_lines.next_line().await
                        } else {
                            out_lines.next_line().await
                        };
                        match rest {
                            Ok(Some(l)) => {
                                rt_out.handle_log_line(&l).await;
                                rt_out.push_log(l).await;
                            }
                            _ => break,
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Reap the child and handle exit / crash restart.
        let mgr = self.clone();
        tokio::spawn(async move {
            let res = child.wait().await;
            mgr.on_process_exit(rt, res).await;
        });
        Ok(())
    }

    /// Send a console command (stdin). Works for vanilla/modded servers.
    pub async fn send_command(&self, id: &str, cmd: &str) -> Result<()> {
        let rt = self.runtime(id).await?;
        let mut guard = rt.stdin.lock().await;
        let stdin = guard
            .as_mut()
            .ok_or_else(|| anyhow!("server is not running"))?;
        let mut line = cmd.trim_end().to_string();
        line.push('\n');
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Graceful stop: send `stop`, wait `shutdown_timeout_sec`, then kill.
    pub async fn stop(&self, id: &str) -> Result<()> {
        let rt = self.runtime(id).await?;
        if !rt.status().await.is_active() {
            return Ok(());
        }
        rt.manual_stop.store(true, Ordering::SeqCst);
        self.set_status(&rt, ServerStatus::Stopping).await;
        let timeout = rt.record.read().await.shutdown_timeout_sec.max(1) as u64;
        drop(self.send_command(id, "stop").await);
        let deadline = Instant::now() + Duration::from_secs(timeout);
        loop {
            if !rt.status().await.is_active() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                rt.push_log("[mcst] stop timed out; killing process".into())
                    .await;
                return self.kill(id).await;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Force-kill the child process group.
    pub async fn kill(&self, id: &str) -> Result<()> {
        let rt = self.runtime(id).await?;
        rt.manual_stop.store(true, Ordering::SeqCst);
        let pid = rt.child_pid.load(Ordering::SeqCst);
        if pid != 0 {
            kill_pid(pid);
        }
        Ok(())
    }

    pub async fn restart(&self, id: &str) -> Result<()> {
        let rt = self.runtime(id).await?;
        if rt.status().await.is_active() {
            self.stop(id).await?;
            // Wait for the status watcher to observe the exit.
            for _ in 0..200 {
                if !rt.status().await.is_active() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        self.start(id).await
    }

    async fn on_process_exit(
        &self,
        rt: Arc<Runtime>,
        res: std::io::Result<std::process::ExitStatus>,
    ) {
        let code = res.ok().and_then(|s| s.code());
        *rt.last_exit.write().await = code;
        *rt.stdin.lock().await = None;
        rt.child_pid.store(0, Ordering::SeqCst);
        rt.set_stats(0.0, 0);
        rt.players.write().await.names.clear();
        let id = rt.record.read().await.id.clone();
        rt.push_log(format!("[mcst] process exited (code {:?})", code))
            .await;

        let manual = rt.manual_stop.load(Ordering::SeqCst);
        let restart = rt.record.read().await.restart_on_crash != 0;
        let crashed = code.map(|c| c != 0).unwrap_or(!manual);

        if crashed && restart && !manual {
            let attempts = rt.crash_restarts.fetch_add(1, Ordering::SeqCst) + 1;
            if attempts <= MAX_CRASH_RESTARTS {
                self.set_status(&rt, ServerStatus::Crashed).await;
                rt.push_log(format!(
                    "[mcst] crashed; restarting in {}s (attempt {attempts}/{MAX_CRASH_RESTARTS})",
                    CRASH_RESTART_DELAY.as_secs()
                ))
                .await;
                let _ = self.inner.restart_tx.send(id.clone());
                return;
            }
            rt.push_log("[mcst] crash-restart limit reached; staying down".into())
                .await;
        }
        let status = if crashed && !manual {
            ServerStatus::Crashed
        } else {
            ServerStatus::Stopped
        };
        self.set_status(&rt, status).await;
        let _ = self
            .inner
            .db
            .audit("", "server_exit", &format!("{id} code={code:?}"))
            .await;
    }

    /// Status + history subscription for the console WS.
    pub async fn console(&self, id: &str) -> Result<(Vec<String>, broadcast::Receiver<String>)> {
        Ok(self.runtime(id).await?.subscribe().await)
    }

    /// Called once at boot: mark auto-start servers for launch.
    pub async fn autostart(&self) {
        match self.inner.db.list_servers().await {
            Ok(recs) => {
                for rec in recs {
                    if rec.auto_start != 0 {
                        let id = rec.id.clone();
                        let mgr = self.clone();
                        tokio::spawn(async move {
                            if let Err(e) = mgr.start(&id).await {
                                tracing::warn!(server = %id, "auto-start failed: {e}");
                            }
                        });
                    }
                }
            }
            Err(e) => tracing::warn!("autostart scan failed: {e}"),
        }
    }

    /// Stats ticker: refresh sysinfo for running servers, broadcast stats.
    pub fn spawn_stats_loop(&self) {
        let mgr = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(2));
            loop {
                tick.tick().await;
                mgr.refresh_stats().await;
            }
        });
    }

    async fn refresh_stats(&self) {
        let rts: Vec<Arc<Runtime>> = self.inner.runtimes.lock().await.values().cloned().collect();
        let running: Vec<_> = {
            let mut v = Vec::new();
            for rt in rts {
                if matches!(
                    *rt.status.read().await,
                    ServerStatus::Running | ServerStatus::Starting
                ) {
                    v.push(rt);
                }
            }
            v
        };
        if running.is_empty() {
            return;
        }
        let pids: Vec<sysinfo::Pid> = running
            .iter()
            .map(|r| sysinfo::Pid::from_u32(r.child_pid.load(Ordering::SeqCst)))
            .filter(|p| p.as_u32() != 0)
            .collect();
        {
            let mut sys = self.inner.sysinfo.lock().await;
            sys.refresh_processes_specifics(
                sysinfo::ProcessesToUpdate::Some(&pids),
                true,
                sysinfo::ProcessRefreshKind::nothing()
                    .with_cpu()
                    .with_memory(),
            );
            for rt in &running {
                let pid = rt.child_pid.load(Ordering::SeqCst);
                if pid == 0 {
                    continue;
                }
                if let Some(p) = sys.process(sysinfo::Pid::from_u32(pid)) {
                    let cpu = p.cpu_usage() as f64;
                    let mem = p.memory();
                    rt.set_stats(cpu, mem);
                    self.emit(PanelEvent::stats(&rt.record.read().await.id, cpu, mem))
                        .await;
                }
            }
        }
        // Ping each running server for player counts (best effort).
        for rt in running {
            let rec = rt.record.read().await.clone();
            let mgr = self.clone();
            tokio::spawn(async move {
                if let Some(info) = crate::ping::status("127.0.0.1", rec.port as u16).await {
                    let mut players = rt.players.write().await;
                    players.max = info.players_max;
                    if info.players_online == 0 {
                        players.names.clear();
                    }
                    drop(players);
                    mgr.emit(PanelEvent::players(
                        &rec.id,
                        info.players_online,
                        info.players_max,
                        &[],
                    ))
                    .await;
                }
            });
        }
    }

    /// Empty-server auto-stop: every minute, stop servers that have had zero
    /// players for `empty_stop_minutes`.
    pub fn spawn_empty_stop_loop(&self) {
        let mgr = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(30));
            loop {
                tick.tick().await;
                let rts: Vec<Arc<Runtime>> =
                    mgr.inner.runtimes.lock().await.values().cloned().collect();
                for rt in rts {
                    if *rt.status.read().await != ServerStatus::Running {
                        continue;
                    }
                    let mins = rt.record.read().await.empty_stop_minutes;
                    if mins <= 0 {
                        continue;
                    }
                    let empty = rt.players.read().await.names.is_empty();
                    if !empty {
                        *rt.last_nonempty.write().await = Instant::now();
                        continue;
                    }
                    let idle = rt.last_nonempty.read().await.elapsed();
                    if idle >= Duration::from_secs(mins as u64 * 60) {
                        let id = rt.record.read().await.id.clone();
                        rt.push_log(format!("[mcst] no players for {mins}m — auto-stopping"))
                            .await;
                        if let Err(e) = mgr.stop(&id).await {
                            tracing::warn!(server = %id, "empty-stop failed: {e}");
                        }
                    }
                }
            }
        });
    }

    /// Stop everything — used on shutdown so children are reaped.
    pub async fn stop_all(&self, timeout: Duration) {
        let rts: Vec<Arc<Runtime>> = self.inner.runtimes.lock().await.values().cloned().collect();
        let deadline = Instant::now() + timeout;
        for rt in &rts {
            if rt.status().await.is_active() {
                let id = rt.record.read().await.id.clone();
                rt.manual_stop.store(true, Ordering::SeqCst);
                drop(self.send_command(&id, "stop").await);
            }
        }
        for rt in &rts {
            while rt.status().await.is_active() && Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            if rt.status().await.is_active() {
                kill_pid(rt.child_pid.load(Ordering::SeqCst));
            }
        }
    }
}

impl Runtime {
    /// Update players/status from a parsed log line.
    async fn handle_log_line(&self, line: &str) {
        match logparse::parse_line(line) {
            logparse::LogEvent::ServerReady => {
                *self.status.write().await = ServerStatus::Running;
                *self.last_nonempty.write().await = Instant::now();
            }
            logparse::LogEvent::Joined(name) => {
                let mut p = self.players.write().await;
                p.names.insert(name);
            }
            logparse::LogEvent::Left(name) => {
                self.players.write().await.names.remove(&name);
            }
            logparse::LogEvent::PlayerList { online, max, names } => {
                let mut p = self.players.write().await;
                p.max = max;
                if !names.is_empty() {
                    p.names = names.into_iter().collect();
                }
                if online == 0 {
                    p.names.clear();
                }
            }
            _ => {}
        }
    }
}

/// Kill a pid cross-platform.
fn kill_pid(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F", "/T"])
            .output();
    }
}

/// A new server request from the create wizard.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateServer {
    pub name: String,
    pub server_type: String,
    pub mc_version: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default = "default_port")]
    pub port: i64,
    #[serde(default = "default_mem")]
    pub memory_mb: i64,
    #[serde(default)]
    pub min_memory_mb: i64,
    #[serde(default)]
    pub jvm_args: String,
    #[serde(default)]
    pub java_path: String,
    #[serde(default)]
    pub accept_eula: bool,
}

fn default_port() -> i64 {
    25565
}
fn default_mem() -> i64 {
    2048
}

impl CreateServer {
    pub fn validate(&self) -> Result<(), String> {
        command::validate_server_name(&self.name)?;
        if crate::servers::types::ServerType::parse(&self.server_type).is_none() {
            return Err(format!("unknown server type '{}'", self.server_type));
        }
        if self.mc_version.trim().is_empty() && self.server_type != "custom" {
            return Err("mc_version is required".into());
        }
        if !(1024..=65535).contains(&self.port) {
            return Err("port must be 1024-65535".into());
        }
        if !(256..=262144).contains(&self.memory_mb) {
            return Err("memory_mb must be 256-262144".into());
        }
        if !self.accept_eula && self.server_type != "custom" {
            return Err("the Minecraft EULA must be accepted".into());
        }
        Ok(())
    }

    pub fn to_record(&self, servers_dir: &std::path::Path) -> ServerRecord {
        let dir = servers_dir
            .join(command::slugify(&self.name))
            .join(Uuid::new_v4().to_string().split('-').next().unwrap_or("x"));
        ServerRecord {
            id: Uuid::new_v4().to_string(),
            name: self.name.clone(),
            server_type: self.server_type.clone(),
            mc_version: self.mc_version.clone(),
            loader_version: self.loader_version.clone(),
            port: self.port,
            memory_mb: self.memory_mb,
            min_memory_mb: self.min_memory_mb,
            java_path: if self.java_path.is_empty() {
                "java".into()
            } else {
                self.java_path.clone()
            },
            jvm_args: self.jvm_args.clone(),
            dir: dir.to_string_lossy().into_owned(),
            jar: "server.jar".into(),
            auto_start: 0,
            restart_on_crash: 0,
            shutdown_timeout_sec: 30,
            empty_stop_minutes: 0,
            icon: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_validate() {
        let good = CreateServer {
            name: "SMP".into(),
            server_type: "paper".into(),
            mc_version: "1.21".into(),
            loader_version: "".into(),
            port: 25565,
            memory_mb: 2048,
            min_memory_mb: 0,
            jvm_args: "".into(),
            java_path: "".into(),
            accept_eula: true,
        };
        assert!(good.validate().is_ok());

        let mut bad = good.clone();
        bad.accept_eula = false;
        assert!(bad.validate().is_err());
        let mut bad = good.clone();
        bad.port = 80;
        assert!(bad.validate().is_err());
        let mut bad = good.clone();
        bad.memory_mb = 10;
        assert!(bad.validate().is_err());
        let mut bad = good.clone();
        bad.server_type = "nope".into();
        assert!(bad.validate().is_err());
        let mut bad = good.clone();
        bad.name = "".into();
        assert!(bad.validate().is_err());
        let mut bad = good.clone();
        bad.mc_version = " ".into();
        assert!(bad.validate().is_err());
    }

    #[test]
    fn create_to_record_slug_dir() {
        let c = CreateServer {
            name: "My SMP".into(),
            server_type: "fabric".into(),
            mc_version: "1.21".into(),
            loader_version: "0.16.0".into(),
            port: 25566,
            memory_mb: 4096,
            min_memory_mb: 1024,
            jvm_args: "-XX:+UseZGC".into(),
            java_path: "/usr/bin/java".into(),
            accept_eula: true,
        };
        let r = c.to_record(&PathBuf::from("/data/servers"));
        assert!(r.dir.starts_with("/data/servers/my-smp/"));
        assert_eq!(r.java_path, "/usr/bin/java");
        assert_eq!(r.jar, "server.jar");
        assert_eq!(r.port, 25566);
    }

    #[test]
    fn log_line_handling() {
        let rt = Runtime::new(ServerRecord {
            id: "x".into(),
            name: "x".into(),
            server_type: "paper".into(),
            mc_version: "1.21".into(),
            loader_version: "".into(),
            port: 25565,
            memory_mb: 2048,
            min_memory_mb: 0,
            java_path: "java".into(),
            jvm_args: "".into(),
            dir: "/x".into(),
            jar: "server.jar".into(),
            auto_start: 0,
            restart_on_crash: 0,
            shutdown_timeout_sec: 30,
            empty_stop_minutes: 0,
            icon: "".into(),
            created_at: "".into(),
            updated_at: "".into(),
        });
        let rt = Arc::new(rt);
        tokio_test::block_on(async {
            rt.handle_log_line("[t] [a/INFO]: Steve joined the game")
                .await;
            assert!(rt.players.read().await.names.contains("Steve"));
            rt.handle_log_line("[t] [a/INFO]: Done (1.0s)!").await;
            assert_eq!(*rt.status.read().await, ServerStatus::Running);
            rt.handle_log_line("[t] [a/INFO]: There are 1 of a max of 20 players online: Steve")
                .await;
            assert_eq!(rt.players.read().await.max, 20);
            rt.handle_log_line("[t] [a/INFO]: Steve left the game")
                .await;
            assert!(rt.players.read().await.names.is_empty());
        });
    }
}
