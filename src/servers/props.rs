//! `server.properties` parsing and editing that preserves comments and
//! ordering — the file is user-facing, so a rewrite must not scramble it.

use std::path::Path;

use anyhow::Result;

/// A single line of a server.properties file.
#[derive(Debug, Clone, PartialEq)]
enum Line {
    Comment(String),
    Blank,
    KeyValue(String, String),
}

/// An editable server.properties document.
#[derive(Debug, Clone, Default)]
pub struct Properties {
    lines: Vec<Line>,
}

impl Properties {
    pub fn parse(text: &str) -> Self {
        let lines = text
            .lines()
            .map(|l| {
                let t = l.trim_end();
                if t.is_empty() {
                    Line::Blank
                } else if t.starts_with('#') || t.starts_with('!') {
                    Line::Comment(t.to_string())
                } else if let Some((k, v)) = t.split_once('=') {
                    Line::KeyValue(k.trim().to_string(), v.trim().to_string())
                } else {
                    // A bare word is kept as a comment so we never lose data.
                    Line::Comment(t.to_string())
                }
            })
            .collect();
        Self { lines }
    }

    pub async fn load(path: &Path) -> Result<Self> {
        match tokio::fs::read_to_string(path).await {
            Ok(s) => Ok(Self::parse(&s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        for l in &self.lines {
            match l {
                Line::Comment(c) => {
                    out.push_str(c);
                    out.push('\n');
                }
                Line::Blank => out.push('\n'),
                Line::KeyValue(k, v) => {
                    out.push_str(k);
                    out.push('=');
                    out.push_str(v);
                    out.push('\n');
                }
            }
        }
        out
    }

    pub async fn save(&self, path: &Path) -> Result<()> {
        if let Some(p) = path.parent() {
            tokio::fs::create_dir_all(p).await.ok();
        }
        tokio::fs::write(path, self.serialize()).await?;
        Ok(())
    }

    /// Get a property value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().find_map(|l| match l {
            Line::KeyValue(k, v) if k == key => Some(v.as_str()),
            _ => None,
        })
    }

    /// Set a property, updating in place or appending.
    pub fn set(&mut self, key: &str, value: &str) {
        for l in &mut self.lines {
            if let Line::KeyValue(k, v) = l {
                if k == key {
                    *v = value.to_string();
                    return;
                }
            }
        }
        self.lines
            .push(Line::KeyValue(key.to_string(), value.to_string()));
    }

    /// Export every key/value pair.
    pub fn entries(&self) -> Vec<(String, String)> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                Line::KeyValue(k, v) => Some((k.clone(), v.clone())),
                _ => None,
            })
            .collect()
    }

    /// Apply a batch of key/value updates.
    pub fn apply(&mut self, entries: &[(String, String)]) {
        for (k, v) in entries {
            self.set(k, v);
        }
    }
}

/// Sensible defaults for a brand-new server.properties.
pub fn defaults(name: &str, port: i64) -> Properties {
    let mut p = Properties::default();
    p.lines.push(Line::Comment(
        "#Minecraft server properties".to_string(),
    ));
    for (k, v) in [
        ("server-port", port.to_string()),
        ("motd", format!("{name} — managed by mcst")),
        ("max-players", "20".into()),
        ("online-mode", "true".into()),
        ("difficulty", "normal".into()),
        ("gamemode", "survival".into()),
        ("pvp", "true".into()),
        ("spawn-protection", "16".into()),
        ("view-distance", "10".into()),
        ("simulation-distance", "10".into()),
        ("enable-command-block", "false".into()),
        ("white-list", "false".into()),
    ] {
        p.set(k, &v);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kv_comments_blanks() {
        let p = Properties::parse("# c\n\na=1\nb=two = with=eq\n!bang\nbare\n");
        assert_eq!(p.get("a"), Some("1"));
        assert_eq!(p.get("b"), Some("two = with=eq"));
        assert!(p.get("missing").is_none());
        // 1 comment, 1 blank, 2 kv, 1 comment(!), 1 comment(bare)
        assert_eq!(p.lines.len(), 6);
    }

    #[test]
    fn set_updates_in_place_and_appends() {
        let mut p = Properties::parse("a=1\n");
        p.set("a", "2");
        p.set("b", "3");
        assert_eq!(p.serialize(), "a=2\nb=3\n");
    }

    #[test]
    fn serialize_preserves_comments_and_order() {
        let text = "#head\nx=1\n\n#tail\ny=2\n";
        let p = Properties::parse(text);
        assert_eq!(p.serialize(), text);
    }

    #[test]
    fn entries_and_apply() {
        let mut p = Properties::parse("a=1\n");
        p.apply(&[("a".into(), "9".into()), ("z".into(), "w".into())]);
        let e = p.entries();
        assert!(e.contains(&("a".into(), "9".into())));
        assert!(e.contains(&("z".into(), "w".into())));
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn defaults_include_port_and_motd() {
        let p = defaults("TestSrv", 25565);
        assert_eq!(p.get("server-port"), Some("25565"));
        assert!(p.get("motd").unwrap().contains("TestSrv"));
        assert_eq!(p.get("online-mode"), Some("true"));
    }

    #[tokio::test]
    async fn load_missing_is_empty_and_save_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.properties");
        let p = Properties::load(&path).await.unwrap();
        assert!(p.entries().is_empty());
        let mut p = Properties::parse("a=1\n");
        p.set("b", "2");
        p.save(&path).await.unwrap();
        let back = Properties::load(&path).await.unwrap();
        assert_eq!(back.get("a"), Some("1"));
        assert_eq!(back.get("b"), Some("2"));
        // save creates parents
        let nested = dir.path().join("x/y/server.properties");
        p.save(&nested).await.unwrap();
        assert!(nested.exists());
    }
}
