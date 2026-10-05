//! Server records and runtime status types.

use serde::{Deserialize, Serialize};

/// Persistent server row (`servers` table).
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ServerRecord {
    pub id: String,
    pub name: String,
    pub server_type: String,
    pub mc_version: String,
    pub loader_version: String,
    pub port: i64,
    pub memory_mb: i64,
    pub min_memory_mb: i64,
    pub java_path: String,
    pub jvm_args: String,
    pub dir: String,
    pub jar: String,
    pub auto_start: i64,
    pub restart_on_crash: i64,
    pub shutdown_timeout_sec: i64,
    pub empty_stop_minutes: i64,
    pub icon: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Live process status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    Stopped,
    Installing,
    Starting,
    Running,
    Stopping,
    Crashed,
}

impl ServerStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Stopping)
    }
    /// Active or mid-install — blocks delete/edit/update-jar.
    pub fn is_busy(self) -> bool {
        self.is_active() || self == Self::Installing
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Installing => "installing",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Crashed => "crashed",
        }
    }
}

/// What the dashboard/detail pages consume: record + live runtime fields.
#[derive(Debug, Clone, Serialize)]
pub struct ServerDto {
    #[serde(flatten)]
    pub record: ServerRecord,
    pub status: ServerStatus,
    pub players_online: i64,
    pub players_max: i64,
    pub player_names: Vec<String>,
    pub cpu_percent: f64,
    pub mem_bytes: u64,
    pub uptime_sec: u64,
    pub last_exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PanelEvent {
    pub kind: String,
    pub server_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl PanelEvent {
    pub fn status(server_id: &str, status: ServerStatus) -> Self {
        Self {
            kind: "server_status".into(),
            server_id: server_id.into(),
            data: Some(serde_json::json!({ "status": status })),
        }
    }
    pub fn stats(server_id: &str, cpu: f64, mem: u64) -> Self {
        Self {
            kind: "server_stats".into(),
            server_id: server_id.into(),
            data: Some(serde_json::json!({ "cpu_percent": cpu, "mem_bytes": mem })),
        }
    }
    pub fn players(server_id: &str, online: i64, max: i64, names: &[String]) -> Self {
        Self {
            kind: "server_players".into(),
            server_id: server_id.into(),
            data: Some(serde_json::json!({
                "online": online, "max": max, "names": names
            })),
        }
    }
    /// A non-server-scoped event (java installs, system changes).
    pub fn system(kind: &str, data: serde_json::Value) -> Self {
        Self {
            kind: kind.into(),
            server_id: String::new(),
            data: Some(data),
        }
    }
}

/// Supported server software types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerType {
    Vanilla,
    Paper,
    Purpur,
    Fabric,
    Forge,
    NeoForge,
    Custom,
}

impl ServerType {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "vanilla" => Self::Vanilla,
            "paper" => Self::Paper,
            "purpur" => Self::Purpur,
            "fabric" => Self::Fabric,
            "forge" => Self::Forge,
            "neoforge" => Self::NeoForge,
            "custom" => Self::Custom,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Paper => "paper",
            Self::Purpur => "purpur",
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Custom => "custom",
        }
    }
    pub fn all() -> &'static [ServerType] {
        &[
            Self::Vanilla,
            Self::Paper,
            Self::Purpur,
            Self::Fabric,
            Self::Forge,
            Self::NeoForge,
            Self::Custom,
        ]
    }
    /// Whether the type supports Modrinth content (mods/plugins).
    pub fn modrinth_facets(self) -> Option<&'static str> {
        match self {
            Self::Vanilla | Self::Custom => None,
            Self::Paper | Self::Purpur => Some("paper"),
            Self::Fabric => Some("fabric"),
            Self::Forge => Some("forge"),
            Self::NeoForge => Some("neoforge"),
        }
    }
    /// Directory that receives installed content.
    pub fn content_dir(self) -> &'static str {
        match self {
            Self::Paper | Self::Purpur => "plugins",
            _ => "mods",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip_and_labels() {
        for s in [
            ServerStatus::Stopped,
            ServerStatus::Installing,
            ServerStatus::Starting,
            ServerStatus::Running,
            ServerStatus::Stopping,
            ServerStatus::Crashed,
        ] {
            let j = serde_json::to_string(&s).unwrap();
            let back: ServerStatus = serde_json::from_str(&j).unwrap();
            assert_eq!(back, s);
            assert!(!s.label().is_empty());
        }
        assert!(ServerStatus::Running.is_active());
        assert!(ServerStatus::Starting.is_active());
        assert!(ServerStatus::Stopping.is_active());
        assert!(!ServerStatus::Stopped.is_active());
        assert!(!ServerStatus::Crashed.is_active());
        assert!(!ServerStatus::Installing.is_active());
    }

    #[test]
    fn server_type_parse_all() {
        for t in ServerType::all() {
            assert_eq!(ServerType::parse(t.as_str()), Some(*t));
        }
        assert_eq!(ServerType::parse("nope"), None);
        assert_eq!(ServerType::parse(""), None);
        assert_eq!(ServerType::Paper.modrinth_facets(), Some("paper"));
        assert_eq!(ServerType::Paper.content_dir(), "plugins");
        assert_eq!(ServerType::Fabric.content_dir(), "mods");
        assert!(ServerType::Vanilla.modrinth_facets().is_none());
        assert_eq!(ServerType::NeoForge.modrinth_facets(), Some("neoforge"));
        assert_eq!(ServerType::Purpur.modrinth_facets(), Some("paper"));
    }

    #[test]
    fn events_serialize() {
        let e = PanelEvent::status("s1", ServerStatus::Running);
        let j = serde_json::to_value(&e).unwrap();
        assert_eq!(j["kind"], "server_status");
        assert_eq!(j["server_id"], "s1");
        assert_eq!(j["data"]["status"], "running");

        let e = PanelEvent::stats("s1", 12.5, 1024);
        assert_eq!(serde_json::to_value(&e).unwrap()["data"]["mem_bytes"], 1024);

        let e = PanelEvent::players("s1", 2, 20, &["a".into(), "b".into()]);
        let j = serde_json::to_value(&e).unwrap();
        assert_eq!(j["data"]["online"], 2);
        assert_eq!(j["data"]["names"].as_array().unwrap().len(), 2);

        let e = PanelEvent::system("java", serde_json::json!({"x":1}));
        assert_eq!(serde_json::to_value(&e).unwrap()["server_id"], "");
    }
}
