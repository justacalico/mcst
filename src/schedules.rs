//! Scheduled tasks: interval and daily timers per server (restart, backup,
//! console command, start/stop).

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Local, Timelike};

use crate::db::{Db, ScheduleRow};
use crate::servers::ServerManager;

/// Is `s` due right now? `last_run_at` is UTC; `daily_time` is local HH:MM.
pub fn is_due(s: &ScheduleRow, now: &DateTime<Local>) -> bool {
    if s.enabled == 0 {
        return false;
    }
    let last: Option<DateTime<Local>> = s
        .last_run_at
        .as_deref()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.with_timezone(&Local))
        .or_else(|| {
            // SQLite timestamps ("Y-m-d H:M:S.fZ") — parse naive as UTC.
            s.last_run_at.as_deref().and_then(|t| {
                chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.fZ")
                    .ok()
                    .map(|n| n.and_utc().with_timezone(&Local))
            })
        });
    if s.every_minutes > 0 {
        return match last {
            None => true, // never ran → due immediately
            Some(l) => (*now - l).num_seconds() >= s.every_minutes * 60,
        };
    }
    if !s.daily_time.is_empty() {
        let (h, m) = match parse_hhmm(&s.daily_time) {
            Some(v) => v,
            None => return false,
        };
        let today_due = now.hour() == h && now.minute() == m;
        if !today_due {
            return false;
        }
        // Due once per day: ran already today → not due.
        return match last {
            None => true,
            Some(l) => l.date_naive() < now.date_naive(),
        };
    }
    false
}

/// Parse "HH:MM".
pub fn parse_hhmm(s: &str) -> Option<(u32, u32)> {
    let (h, m) = s.trim().split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some((h, m))
}

/// Validate a schedule request before insert/update.
pub fn validate(
    action: &str,
    payload: &str,
    every_minutes: i64,
    daily_time: &str,
) -> Result<(), String> {
    match action {
        "command" if payload.trim().is_empty() => {
            return Err("command schedules need a command".into())
        }
        "command" | "start" | "stop" | "restart" | "backup" => {}
        _ => return Err(format!("unknown action '{action}'")),
    }
    if every_minutes == 0 && daily_time.is_empty() {
        return Err("set an interval or a daily time".into());
    }
    if every_minutes != 0 && !(1..=43200).contains(&every_minutes) {
        return Err("interval must be 1-43200 minutes".into());
    }
    if !daily_time.is_empty() && parse_hhmm(daily_time).is_none() {
        return Err("daily_time must be HH:MM".into());
    }
    if every_minutes > 0 && !daily_time.is_empty() {
        return Err("pick either an interval or a daily time, not both".into());
    }
    Ok(())
}

/// Run one schedule against its server.
pub async fn execute(mgr: &ServerManager, db: &Db, s: &ScheduleRow) -> Result<()> {
    match s.action.as_str() {
        "command" => mgr.send_command(&s.server_id, &s.payload).await?,
        "start" => mgr.start(&s.server_id).await?,
        "stop" => mgr.stop(&s.server_id).await?,
        "restart" => mgr.restart(&s.server_id).await?,
        "backup" => {
            let rec = db
                .get_server(&s.server_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("server gone"))?;
            crate::backups::create(
                db,
                &s.server_id,
                std::path::Path::new(&rec.dir),
                &mgr.backups_dir(),
                &s.payload,
            )
            .await?;
            let keep = db
                .get_setting("backup_keep")
                .await
                .ok()
                .flatten()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(10);
            crate::backups::prune(db, &s.server_id, keep).await.ok();
        }
        _ => {}
    }
    db.mark_schedule_run(&s.id).await.ok();
    db.audit("", "schedule_run", &format!("{} ({})", s.name, s.action))
        .await
        .ok();
    Ok(())
}

/// The ticking loop — checks every 30s, runs due schedules.
pub fn spawn_loop(mgr: ServerManager, db: Db) {
    let mgr = Arc::new(mgr);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        loop {
            tick.tick().await;
            let now = Local::now();
            let Ok(list) = db.all_schedules().await else {
                continue;
            };
            for s in list {
                if !is_due(&s, &now) {
                    continue;
                }
                let (m, d) = (mgr.clone(), db.clone());
                tokio::spawn(async move {
                    if let Err(e) = execute(&m, &d, &s).await {
                        tracing::warn!(schedule = %s.name, "schedule failed: {e}");
                    }
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sched(every: i64, daily: &str, last: Option<&str>) -> ScheduleRow {
        ScheduleRow {
            id: "x".into(),
            server_id: "s".into(),
            name: "t".into(),
            action: "restart".into(),
            payload: "".into(),
            every_minutes: every,
            daily_time: daily.into(),
            enabled: 1,
            last_run_at: last.map(str::to_string),
            created_at: "".into(),
        }
    }

    #[test]
    fn interval_due_logic() {
        let now = Local::now();
        // Never ran → due.
        assert!(is_due(&sched(30, "", None), &now));
        // Ran just now → not due.
        let just = now.to_rfc3339();
        assert!(!is_due(&sched(30, "", Some(&just)), &now));
        // Ran an hour ago with 30m interval → due.
        let old = (now - chrono::Duration::hours(1)).to_rfc3339();
        assert!(is_due(&sched(30, "", Some(&old)), &now));
        // Disabled → never due.
        let mut s = sched(30, "", None);
        s.enabled = 0;
        assert!(!is_due(&s, &now));
    }

    #[test]
    fn daily_due_logic() {
        let now = Local::now();
        let hhmm = format!("{:02}:{:02}", now.hour(), now.minute());
        // Due right now, never ran.
        assert!(is_due(&sched(0, &hhmm, None), &now));
        // Already ran today → not due.
        let today = now.to_rfc3339();
        assert!(!is_due(&sched(0, &hhmm, Some(&today)), &now));
        // Ran yesterday → due.
        let yesterday = (now - chrono::Duration::days(1)).to_rfc3339();
        assert!(is_due(&sched(0, &hhmm, Some(&yesterday)), &now));
        // Different time → not due.
        let other = format!("{:02}:{:02}", (now.hour() + 12) % 24, now.minute());
        assert!(!is_due(&sched(0, &other, None), &now));
        // Bad format → not due.
        assert!(!is_due(&sched(0, "25:99", None), &now));
    }

    #[test]
    fn hhmm_parse() {
        assert_eq!(parse_hhmm("04:30"), Some((4, 30)));
        assert_eq!(parse_hhmm("23:59"), Some((23, 59)));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("12:60"), None);
        assert_eq!(parse_hhmm("x"), None);
        assert_eq!(parse_hhmm(""), None);
    }

    #[test]
    fn validate_rules() {
        assert!(validate("restart", "", 30, "").is_ok());
        assert!(validate("backup", "note", 0, "04:00").is_ok());
        assert!(validate("command", "say hi", 60, "").is_ok());
        assert!(validate("command", "", 60, "").is_err());
        assert!(validate("bogus", "", 60, "").is_err());
        assert!(validate("restart", "", 0, "").is_err());
        assert!(validate("restart", "", -5, "").is_err());
        assert!(validate("restart", "", 999999, "").is_err());
        assert!(validate("restart", "", 0, "9:99").is_err());
        assert!(validate("restart", "", 30, "04:00").is_err());
    }
}
