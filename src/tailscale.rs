//! Tailscale integration: status reporting plus `tailscale serve` to expose
//! the panel (HTTPS) and game ports (TCP) on the tailnet.

use std::time::Duration;

use serde::Serialize;
use tokio::process::Command;

#[derive(Clone)]
pub struct Tailscale {
    bin: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TailscaleStatus {
    pub installed: bool,
    pub running: bool,
    pub ip: String,
    pub hostname: String,
    pub tailnet: String,
    pub panel_serve_port: u16,
}

const CMD_TIMEOUT: Duration = Duration::from_secs(10);

impl Tailscale {
    pub fn new(bin: String) -> Self {
        Self { bin }
    }

    /// Run a tailscale CLI command with a timeout.
    async fn run(&self, args: &[&str]) -> std::io::Result<std::process::Output> {
        tokio::time::timeout(CMD_TIMEOUT, Command::new(&self.bin).args(args).output())
            .await
            .unwrap_or_else(|_| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "tailscale timed out",
                ))
            })
    }

    /// Is the `tailscale` CLI on PATH?
    pub async fn installed(&self) -> bool {
        which::which(&self.bin).is_ok()
    }

    /// Parse `tailscale status --json` output.
    pub fn parse_status(json: &str) -> (String, String, String) {
        let doc: serde_json::Value = match serde_json::from_str(json) {
            Ok(v) => v,
            Err(_) => return (String::new(), String::new(), String::new()),
        };
        let ip = doc
            .pointer("/Self/TailscaleIPs/0")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let hostname = doc
            .pointer("/Self/DNSName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim_end_matches('.')
            .to_string();
        let tailnet = doc
            .pointer("/CurrentTailnet/Name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        (ip, hostname, tailnet)
    }

    /// Full status snapshot.
    pub async fn status(&self, panel_serve_port: u16) -> TailscaleStatus {
        let mut st = TailscaleStatus {
            panel_serve_port,
            ..Default::default()
        };
        if !self.installed().await {
            return st;
        }
        st.installed = true;
        let Ok(out) = self.run(&["status", "--json"]).await else {
            return st;
        };
        if !out.status.success() {
            return st;
        }
        st.running = true;
        let text = String::from_utf8_lossy(&out.stdout);
        let (ip, host, net) = Self::parse_status(&text);
        st.ip = ip;
        st.hostname = host;
        st.tailnet = net;
        st
    }

    /// Serve the panel over HTTPS on the tailnet (`tailscale serve`).
    pub async fn serve_panel(&self, backend_port: u16, https_port: u16) -> anyhow::Result<()> {
        let target = format!("http://127.0.0.1:{backend_port}");
        let out = self
            .run(&["serve", "--bg", &format!("--https={https_port}"), &target])
            .await?;
        if !out.status.success() {
            anyhow::bail!(
                "tailscale serve failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }

    /// Stop serving the panel.
    pub async fn unserve_panel(&self, https_port: u16) -> anyhow::Result<()> {
        let out = self
            .run(&["serve", "--bg", &format!("--https={https_port}"), "off"])
            .await?;
        if !out.status.success() {
            anyhow::bail!(
                "tailscale serve off failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }

    /// Expose a Minecraft TCP port on the tailnet at `tailnet_port`
    /// (friends connect to `<node>.<tailnet>:<tailnet_port>`).
    pub async fn serve_tcp(&self, server_port: u16, tailnet_port: u16) -> anyhow::Result<()> {
        let target = format!("tcp://127.0.0.1:{server_port}");
        let out = self
            .run(&["serve", "--bg", &format!("--tcp={tailnet_port}"), &target])
            .await?;
        if !out.status.success() {
            anyhow::bail!(
                "tailscale serve tcp failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }

    pub async fn unserve_tcp(&self, tailnet_port: u16) -> anyhow::Result<()> {
        let out = self
            .run(&["serve", "--bg", &format!("--tcp={tailnet_port}"), "off"])
            .await?;
        if !out.status.success() {
            anyhow::bail!(
                "tailscale serve tcp off failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }
}

/// Settings keys.
pub const KEY_PANEL_SERVE: &str = "tailscale_panel_serve";
pub const KEY_PANEL_SERVE_PORT: &str = "tailscale_panel_serve_port";
pub const DEFAULT_HTTPS_PORT: u16 = 443;

/// Per-server setting key for tailscale TCP exposure.
pub fn server_tcp_key(server_id: &str) -> String {
    format!("tailscale_tcp_{server_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_parsing() {
        let (ip, host, net) = Tailscale::parse_status(
            r#"{"Self":{"TailscaleIPs":["100.64.1.2","fd7a::1"],"DNSName":"myhost.tailnet.ts.net."},
                "CurrentTailnet":{"Name":"example.com"}}"#,
        );
        assert_eq!(ip, "100.64.1.2");
        assert_eq!(host, "myhost.tailnet.ts.net");
        assert_eq!(net, "example.com");

        let (ip, host, net) = Tailscale::parse_status("{}");
        assert!(ip.is_empty() && host.is_empty() && net.is_empty());
        let (ip, _, _) = Tailscale::parse_status("not json");
        assert!(ip.is_empty());
    }

    #[test]
    fn keys() {
        assert_eq!(server_tcp_key("abc"), "tailscale_tcp_abc");
        assert_eq!(KEY_PANEL_SERVE, "tailscale_panel_serve");
    }

    #[tokio::test]
    async fn not_installed_status() {
        let ts = Tailscale::new("definitely-not-tailscale".into());
        assert!(!ts.installed().await);
        let st = ts.status(443).await;
        assert!(!st.installed && !st.running);
        assert!(ts.serve_panel(25580, 443).await.is_err());
    }
}
