//! Minecraft server log-line parsing: player joins/leaves, boot detection,
//! `list` command output.

use once_cell::sync::Lazy;
use regex::Regex;

/// What a single log line tells us.
#[derive(Debug, Clone, PartialEq)]
pub enum LogEvent {
    /// `[Server thread/INFO]: Done (1.23s)! For help, type "help"`
    ServerReady,
    /// `Steve joined the game` / `Steve left the game`
    Joined(String),
    Left(String),
    /// `UUID of player Steve is xxxx`
    Uuid {
        name: String,
        uuid: String,
    },
    /// Output of the `list` command:
    /// `There are 2 of a max of 20 players online: Steve, Alex`
    PlayerList {
        online: i64,
        max: i64,
        names: Vec<String>,
    },
    /// Nothing interesting.
    None,
}

static RE_DONE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"Done \([\d.]+s\)!?"#).unwrap());
static RE_JOIN: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\w{1,16}) joined the game$").unwrap());
static RE_LEAVE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\w{1,16}) (?:left the game|lost connection)").unwrap());
static RE_UUID: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^UUID of player (\w{1,16}) is ([0-9a-fA-F-]+)$").unwrap());
static RE_LIST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^There are (\d+) of a max of (\d+) players online:?\s*(.*)$").unwrap()
});

/// Strip the `[HH:MM:SS] [thread/LEVEL]:` prefix from a vanilla-style log
/// line, returning the message body.
pub fn message_body(line: &str) -> &str {
    // [12:34:56] [Server thread/INFO]: message
    if let Some(idx) = line.find("]: ") {
        let head = &line[..idx];
        if head.starts_with('[') {
            return &line[idx + 3..];
        }
    }
    line
}

pub fn parse_line(line: &str) -> LogEvent {
    let body = message_body(line);
    if RE_DONE.is_match(body) {
        return LogEvent::ServerReady;
    }
    if let Some(c) = RE_UUID.captures(body) {
        return LogEvent::Uuid {
            name: c[1].to_string(),
            uuid: c[2].to_string(),
        };
    }
    if let Some(c) = RE_JOIN.captures(body) {
        return LogEvent::Joined(c[1].to_string());
    }
    if let Some(c) = RE_LEAVE.captures(body) {
        return LogEvent::Left(c[1].to_string());
    }
    if let Some(c) = RE_LIST.captures(body) {
        let names: Vec<String> = c[3]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        return LogEvent::PlayerList {
            online: c[1].parse().unwrap_or(0),
            max: c[2].parse().unwrap_or(0),
            names,
        };
    }
    LogEvent::None
}

/// A line is an error/fatal worth highlighting.
pub fn is_error_line(line: &str) -> bool {
    line.contains("/ERROR]") || line.contains("/FATAL]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn done_detection() {
        let l = "[12:00:00] [Server thread/INFO]: Done (2.341s)! For help, type \"help\"";
        assert_eq!(parse_line(l), LogEvent::ServerReady);
        assert_eq!(parse_line("[x] Done"), LogEvent::None);
    }

    #[test]
    fn join_leave_uuid() {
        assert_eq!(
            parse_line("[12:00:00] [Server thread/INFO]: Steve joined the game"),
            LogEvent::Joined("Steve".into())
        );
        assert_eq!(
            parse_line("[12:00:00] [Server thread/INFO]: Alex left the game"),
            LogEvent::Left("Alex".into())
        );
        assert_eq!(
            parse_line("[12:00:00] [Server thread/INFO]: Bob lost connection: quit"),
            LogEvent::Left("Bob".into())
        );
        assert_eq!(
            parse_line(
                "[12:00:00] [Server thread/INFO]: UUID of player Steve is 069a79f4-44e9-4726-a5be-fca90e38aaf5"
            ),
            LogEvent::Uuid {
                name: "Steve".into(),
                uuid: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into()
            }
        );
        // A name over 16 chars is not a player name.
        assert_eq!(
            parse_line("[t] [x/INFO]: ThisIsAVeryLongPlayerName joined the game"),
            LogEvent::None
        );
    }

    #[test]
    fn list_output() {
        let l = "[12:00:00] [Server thread/INFO]: There are 2 of a max of 20 players online: Steve, Alex";
        assert_eq!(
            parse_line(l),
            LogEvent::PlayerList {
                online: 2,
                max: 20,
                names: vec!["Steve".into(), "Alex".into()]
            }
        );
        let empty = parse_line("There are 0 of a max of 20 players online:");
        match empty {
            LogEvent::PlayerList { online, names, .. } => {
                assert_eq!(online, 0);
                assert!(names.is_empty());
            }
            _ => panic!("expected PlayerList"),
        }
    }

    #[test]
    fn message_body_strips_prefix() {
        assert_eq!(message_body("[12:00:00] [a/INFO]: hi"), "hi");
        assert_eq!(message_body("no prefix"), "no prefix");
        assert_eq!(message_body("[t] [a/INFO]: a ]: b"), "a ]: b");
    }

    #[test]
    fn error_lines() {
        assert!(is_error_line("[t] [a/ERROR]: bad"));
        assert!(is_error_line("[t] [a/FATAL]: bad"));
        assert!(!is_error_line("[t] [a/INFO]: fine"));
        assert!(!is_error_line("[t] [a/WARN]: meh"));
    }
}
