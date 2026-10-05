//! Runtime configuration: CLI flags and data-directory resolution. There is
//! deliberately no env-file support — everything a user needs is a flag.

use std::ffi::OsStr;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

pub const DEFAULT_PORT: u16 = 25580;

/// All runtime configuration for an mcst instance.
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    /// Root data directory: database, servers, backups, caches.
    pub data_dir: PathBuf,
    /// `--dev`: random port, in-memory database, no authentication.
    pub dev_mode: bool,
    /// `--local`: confine dev mode to loopback (implies --dev).
    pub local_only: bool,
    /// Path/name of the `tailscale` binary.
    pub tailscale_bin: String,
}

impl Config {
    /// Parse process arguments. Supported flags:
    /// `-p/--port`, `--host`, `-d/--data`, `--dev`, `--local`,
    /// `-h/--help`, `-V/--version`.
    pub fn from_args<I, S>(args: I) -> Result<ConfigAction>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut host = "0.0.0.0".to_string();
        let mut port = DEFAULT_PORT;
        let mut data_dir: Option<PathBuf> = None;
        let mut dev_mode = false;
        let mut local_only = false;

        let mut it = args.into_iter().peekable();
        while let Some(arg) = it.next() {
            let arg = arg.as_ref();
            let arg = arg.to_string_lossy();
            match arg.as_ref() {
                "-h" | "--help" => return Ok(ConfigAction::Help),
                "-V" | "--version" => return Ok(ConfigAction::Version),
                "-p" | "--port" => {
                    let v = it.next().context("--port requires a value")?;
                    port = v
                        .as_ref()
                        .to_string_lossy()
                        .parse::<u16>()
                        .context("--port must be a valid port (0-65535)")?;
                }
                "--host" => {
                    let v = it.next().context("--host requires a value")?;
                    host = v.as_ref().to_string_lossy().into_owned();
                }
                "-d" | "--data" => {
                    let v = it.next().context("--data requires a value")?;
                    data_dir = Some(PathBuf::from(v.as_ref()));
                }
                "--dev" | "-dev" => dev_mode = true,
                "--local" | "-local" => local_only = true,
                a if a.starts_with("--port=") => {
                    port = a["--port=".len()..]
                        .parse::<u16>()
                        .context("--port must be a valid port (0-65535)")?;
                }
                a if a.starts_with("--host=") => {
                    host = a["--host=".len()..].to_string();
                }
                a if a.starts_with("--data=") => {
                    data_dir = Some(PathBuf::from(&a["--data=".len()..]));
                }
                a if a.starts_with('-') => bail!("unknown flag: {a} (try --help)"),
                a => bail!("unexpected argument: {a} (try --help)"),
            }
        }

        let data_dir = data_dir.unwrap_or_else(default_data_dir);
        Ok(ConfigAction::Run(Config {
            host,
            port,
            data_dir,
            dev_mode: dev_mode || local_only,
            local_only,
            tailscale_bin: "tailscale".into(),
        }))
    }

    /// Apply dev mode: loopback or wildcard bind on a random port.
    pub fn apply_dev_mode(&mut self) {
        self.host = if self.local_only {
            "127.0.0.1".into()
        } else {
            "0.0.0.0".into()
        };
        self.port = 0;
    }

    /// The bind address (`host:port`, bracketed IPv6).
    pub fn bind_addr(&self) -> String {
        if self
            .host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_ipv6())
        {
            format!(
                "[{}]:{}",
                self.host.trim_start_matches('[').trim_end_matches(']'),
                self.port
            )
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    /// sqlite connection URL for the panel database.
    pub fn db_url(&self) -> String {
        if self.dev_mode {
            return "sqlite::memory:".into();
        }
        format!(
            "sqlite:{}?mode=rwc",
            self.data_dir.join("mcst.db").display()
        )
    }

    pub fn servers_dir(&self) -> PathBuf {
        self.data_dir.join("servers")
    }
    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }
    /// Downloaded artifacts (jars, installers, java runtimes).
    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("cache")
    }
    pub fn java_dir(&self) -> PathBuf {
        self.data_dir.join("java")
    }
}

/// What the process should do after parsing args.
pub enum ConfigAction {
    Run(Config),
    Help,
    Version,
}

pub fn usage() -> &'static str {
    "mcst — self-hosted Minecraft server manager\n\
     \n\
     USAGE:\n    \
         mcst [OPTIONS]\n\
     \n\
     OPTIONS:\n    \
         -p, --port <PORT>   Port for the web panel [default: 25580]\n    \
             --host <HOST>   Interface to bind [default: 0.0.0.0]\n    \
         -d, --data <DIR>    Data directory [default: platform data dir]\n    \
             --dev           Dev mode: random port, in-memory DB, no auth\n    \
             --local         Dev mode on loopback only\n    \
         -h, --help          Print help\n    \
         -V, --version       Print version"
}

/// Default data directory: `$XDG_DATA_HOME/mcst`, `~/.local/share/mcst`,
/// `%APPDATA%/mcst`, or `./mcst-data` as a last resort.
pub fn default_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(p) = std::env::var_os("APPDATA") {
            return PathBuf::from(p).join("mcst");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(p) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(p).join("mcst");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("mcst");
        }
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("mcst-data")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<ConfigAction> {
        Config::from_args(args.iter().map(std::ffi::OsStr::new))
    }

    fn run_cfg(args: &[&str]) -> Config {
        match parse(args).unwrap() {
            ConfigAction::Run(c) => c,
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn defaults() {
        let c = run_cfg(&[]);
        assert_eq!(c.port, DEFAULT_PORT);
        assert_eq!(c.host, "0.0.0.0");
        assert!(!c.dev_mode);
        assert!(c.data_dir.ends_with("mcst") || c.data_dir.ends_with("mcst-data"));
    }

    #[test]
    fn port_flag_short_long_and_equals() {
        assert_eq!(run_cfg(&["-p", "1234"]).port, 1234);
        assert_eq!(run_cfg(&["--port", "1234"]).port, 1234);
        assert_eq!(run_cfg(&["--port=1234"]).port, 1234);
        assert!(parse(&["-p"]).is_err());
        assert!(parse(&["-p", "abc"]).is_err());
        assert!(parse(&["-p", "99999"]).is_err());
    }

    #[test]
    fn host_and_data_flags() {
        let c = run_cfg(&["--host", "127.0.0.1", "-d", "/tmp/mcst-x"]);
        assert_eq!(c.host, "127.0.0.1");
        assert_eq!(c.data_dir, PathBuf::from("/tmp/mcst-x"));
        let c = run_cfg(&["--host=::1", "--data=/tmp/mcst-y"]);
        assert_eq!(c.host, "::1");
        assert_eq!(c.data_dir, PathBuf::from("/tmp/mcst-y"));
        assert!(parse(&["--host"]).is_err());
        assert!(parse(&["--data"]).is_err());
    }

    #[test]
    fn dev_and_local() {
        let mut c = run_cfg(&["--dev"]);
        assert!(c.dev_mode && !c.local_only);
        c.apply_dev_mode();
        assert_eq!(c.port, 0);
        assert_eq!(c.host, "0.0.0.0");

        let mut c = run_cfg(&["--local"]);
        assert!(c.dev_mode && c.local_only);
        c.apply_dev_mode();
        assert_eq!(c.host, "127.0.0.1");

        let _ = run_cfg(&["-dev"]);
        let _ = run_cfg(&["-local"]);
    }

    #[test]
    fn help_version_unknown() {
        assert!(matches!(parse(&["-h"]).unwrap(), ConfigAction::Help));
        assert!(matches!(parse(&["--help"]).unwrap(), ConfigAction::Help));
        assert!(matches!(parse(&["-V"]).unwrap(), ConfigAction::Version));
        assert!(matches!(
            parse(&["--version"]).unwrap(),
            ConfigAction::Version
        ));
        assert!(parse(&["--bogus"]).is_err());
        assert!(parse(&["stray"]).is_err());
    }

    #[test]
    fn bind_addr_formats() {
        let mut c = run_cfg(&[]);
        c.host = "127.0.0.1".into();
        c.port = 25580;
        assert_eq!(c.bind_addr(), "127.0.0.1:25580");
        c.host = "::1".into();
        assert_eq!(c.bind_addr(), "[::1]:25580");
        c.host = "[::1]".into();
        assert_eq!(c.bind_addr(), "[::1]:25580");
    }

    #[test]
    fn db_url_memory_in_dev_and_file_otherwise() {
        let mut c = run_cfg(&["-d", "/tmp/mcst-db"]);
        assert!(c.db_url().starts_with("sqlite:/tmp/mcst-db/mcst.db"));
        c.dev_mode = true;
        assert_eq!(c.db_url(), "sqlite::memory:");
        assert!(c.servers_dir().ends_with("servers"));
        assert!(c.backups_dir().ends_with("backups"));
        assert!(c.cache_dir().ends_with("cache"));
        assert!(c.java_dir().ends_with("java"));
    }

    #[test]
    fn usage_mentions_flags() {
        let u = usage();
        assert!(u.contains("--port"));
        assert!(u.contains("--data"));
    }
}
