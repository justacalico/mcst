//! Building the `java` command line for a server, including Forge/NeoForge
//! `@unix_args.txt` style launches.

use std::path::{Path, PathBuf};

use crate::servers::types::ServerRecord;

/// Split a JVM argument string honoring double quotes.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let chars = s.chars().peekable();
    for c in chars {
        match c {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Find a Forge/NeoForge `unix_args.txt` inside the server dir's
/// `libraries/` tree (e.g. `libraries/net/minecraftforge/forge/<v>/`).
pub async fn find_args_file(dir: &Path) -> Option<PathBuf> {
    let libs = dir.join("libraries");
    let mut stack = vec![libs];
    while let Some(d) = stack.pop() {
        let mut rd = match tokio::fs::read_dir(&d).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        while let Ok(Some(e)) = rd.next_entry().await {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|n| n == "unix_args.txt") {
                return Some(p);
            }
        }
    }
    None
}

/// Ensure `user_jvm_args.txt` reflects the configured memory and extra
/// args — Forge/NeoForge launchers read it automatically.
async fn write_user_jvm_args(dir: &Path, rec: &ServerRecord) -> std::io::Result<()> {
    let mut content = format!("-Xmx{}M", rec.memory_mb);
    if rec.min_memory_mb > 0 {
        content.push_str(&format!("\n-Xms{}M", rec.min_memory_mb));
    }
    for a in split_args(&rec.jvm_args) {
        content.push('\n');
        content.push_str(&a);
    }
    content.push('\n');
    tokio::fs::write(dir.join("user_jvm_args.txt"), content).await
}

/// Build `(program, args)` for launching the server.
pub async fn build_command(dir: &Path, rec: &ServerRecord) -> (String, Vec<String>) {
    if let Some(args_file) = find_args_file(dir).await {
        // Modern Forge/NeoForge: user_jvm_args.txt + unix_args.txt.
        let _ = write_user_jvm_args(dir, rec).await;
        let rel = args_file
            .strip_prefix(dir)
            .map(|p| p.to_path_buf())
            .unwrap_or(args_file);
        return (
            rec.java_path.clone(),
            vec![
                "@user_jvm_args.txt".to_string(),
                format!("@{}", rel.display()),
                "nogui".to_string(),
            ],
        );
    }
    let mut args = vec![format!("-Xmx{}M", rec.memory_mb)];
    if rec.min_memory_mb > 0 {
        args.push(format!("-Xms{}M", rec.min_memory_mb));
    }
    args.extend(split_args(&rec.jvm_args));
    args.push("-jar".into());
    args.push(rec.jar.clone());
    args.push("nogui".into());
    (rec.java_path.clone(), args)
}

/// Validate that a server name is usable as a directory component.
pub fn validate_server_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() || name.len() > 64 {
        return Err("name must be 1-64 characters");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '.' | '-' | '(' | ')'))
    {
        return Err("name contains unsupported characters");
    }
    Ok(())
}

/// Turn a server name into a filesystem-safe slug for its directory.
pub fn slugify(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else if c == ' ' || c == '.' {
                '-'
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "server".into()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_args_basic() {
        assert_eq!(
            split_args("-Xmx2G -XX:+UseG1GC \"-Dfoo=a b\"  tail"),
            vec!["-Xmx2G", "-XX:+UseG1GC", "-Dfoo=a b", "tail"]
        );
        assert!(split_args("").is_empty());
        assert!(split_args("   ").is_empty());
        assert_eq!(split_args("\"unclosed"), vec!["unclosed"]);
    }

    fn rec(jar: &str) -> ServerRecord {
        ServerRecord {
            id: "id".into(),
            name: "n".into(),
            server_type: "paper".into(),
            mc_version: "1.21".into(),
            loader_version: "".into(),
            port: 25565,
            memory_mb: 4096,
            min_memory_mb: 1024,
            java_path: "java".into(),
            jvm_args: "-XX:+UseZGC".into(),
            dir: "/x".into(),
            jar: jar.into(),
            auto_start: 0,
            restart_on_crash: 0,
            shutdown_timeout_sec: 30,
            empty_stop_minutes: 0,
            icon: "".into(),
            created_at: "".into(),
            updated_at: "".into(),
        }
    }

    #[tokio::test]
    async fn vanilla_style_command() {
        let dir = tempfile::tempdir().unwrap();
        let (prog, args) = build_command(dir.path(), &rec("server.jar")).await;
        assert_eq!(prog, "java");
        assert_eq!(
            args,
            vec![
                "-Xmx4096M",
                "-Xms1024M",
                "-XX:+UseZGC",
                "-jar",
                "server.jar",
                "nogui"
            ]
        );
    }

    #[tokio::test]
    async fn forge_style_command_uses_args_file() {
        let dir = tempfile::tempdir().unwrap();
        let af = dir
            .path()
            .join("libraries/net/minecraftforge/forge/1.21-51.0.0/unix_args.txt");
        tokio::fs::create_dir_all(af.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&af, "-p x").await.unwrap();
        let (prog, args) = build_command(dir.path(), &rec("ignored")).await;
        assert_eq!(prog, "java");
        assert_eq!(args[0], "@user_jvm_args.txt");
        assert!(args[1].starts_with('@'));
        assert!(args[1].contains("unix_args.txt"));
        assert_eq!(args[2], "nogui");
        let uja = tokio::fs::read_to_string(dir.path().join("user_jvm_args.txt"))
            .await
            .unwrap();
        assert!(uja.contains("-Xmx4096M"));
        assert!(uja.contains("-Xms1024M"));
        assert!(uja.contains("-XX:+UseZGC"));
    }

    #[tokio::test]
    async fn find_args_file_returns_none_without_libraries() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_args_file(dir.path()).await.is_none());
        tokio::fs::create_dir_all(dir.path().join("libraries/empty"))
            .await
            .unwrap();
        assert!(find_args_file(dir.path()).await.is_none());
    }

    #[test]
    fn server_name_validation() {
        assert!(validate_server_name("My Server 2").is_ok());
        assert!(validate_server_name("smp_1.0-beta(2)").is_ok());
        assert!(validate_server_name("").is_err());
        assert!(validate_server_name(&"x".repeat(70)).is_err());
        assert!(validate_server_name("bad/name").is_err());
        assert!(validate_server_name("bad\\name").is_err());
    }

    #[test]
    fn slugify_names() {
        assert_eq!(slugify("My Server"), "my-server");
        assert_eq!(slugify("SMP 1.21"), "smp-1-21");
        assert_eq!(slugify("!!!"), "___");
        assert_eq!(slugify("   "), "server");
        assert_eq!(slugify("a"), "a");
    }
}
