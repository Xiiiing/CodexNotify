use crate::error::CoreResult;
use crate::notification::Notification;
use crate::paths::AppPaths;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (1, unixepoch());
CREATE TABLE IF NOT EXISTS events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_key TEXT NOT NULL UNIQUE,
  event_type TEXT NOT NULL,
  session_id TEXT NOT NULL DEFAULT '',
  turn_id TEXT NOT NULL DEFAULT '',
  project TEXT NOT NULL,
  title TEXT NOT NULL,
  subtitle TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  status TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  next_attempt_at INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  sent_at INTEGER,
  error TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_events_due ON events(status, next_attempt_at);
CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at DESC);
"#;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EventStatus {
    Queued,
    Sending,
    Retrying,
    Sent,
    Failed,
    Suppressed,
}

impl EventStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Retrying => "retrying",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Suppressed => "suppressed",
        }
    }
    fn parse(value: &str) -> Self {
        match value {
            "sending" => Self::Sending,
            "retrying" => Self::Retrying,
            "sent" => Self::Sent,
            "failed" => Self::Failed,
            "suppressed" => Self::Suppressed,
            _ => Self::Queued,
        }
    }

    pub fn label(self) -> &'static str {
        self.as_str()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRecord {
    pub id: i64,
    pub event_key: String,
    pub event_type: String,
    pub project: String,
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub status: EventStatus,
    pub attempts: u32,
    pub next_attempt_at: i64,
    pub created_at: i64,
    pub sent_at: Option<i64>,
    pub error: String,
    #[serde(skip_serializing)]
    pub payload_json: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventCounts {
    pub queued: u32,
    pub sending: u32,
    pub retrying: u32,
    pub sent: u32,
    pub failed: u32,
    pub suppressed: u32,
}

#[derive(Clone)]
pub struct EventStore {
    path: std::path::PathBuf,
}

impl EventStore {
    pub fn new(paths: &AppPaths) -> CoreResult<Self> {
        paths.ensure()?;
        let value = Self {
            path: paths.events_db(),
        };
        value.connect()?;
        Ok(value)
    }
    fn connect(&self) -> CoreResult<Connection> {
        let connection = Connection::open(&self.path)?;
        connection.busy_timeout(std::time::Duration::from_secs(10))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.execute_batch(SCHEMA)?;
        Ok(connection)
    }

    pub fn enqueue(&self, notification: &Notification) -> CoreResult<(i64, bool)> {
        let now = Utc::now().timestamp();
        let status = if notification.suppressed {
            EventStatus::Suppressed
        } else {
            EventStatus::Queued
        };
        let connection = self.connect()?;
        let changed = connection.execute(
            "INSERT OR IGNORE INTO events(event_key,event_type,session_id,turn_id,project,title,subtitle,body,payload_json,status,next_attempt_at,created_at,error) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![notification.event_key, notification.event_type, notification.session_id, notification.turn_id, notification.project, notification.title, notification.subtitle, notification.body, serde_json::to_string(notification)?, status.as_str(), now, now, notification.suppress_reason],
        )?;
        let id = connection.query_row(
            "SELECT id FROM events WHERE event_key=?",
            [&notification.event_key],
            |row| row.get(0),
        )?;
        Ok((id, changed == 1))
    }

    pub fn claim_due(&self, limit: u32) -> CoreResult<Vec<EventRecord>> {
        let now = Utc::now().timestamp();
        let mut connection = self.connect()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        transaction.execute(
            "UPDATE events SET status='retrying' WHERE status='sending' AND next_attempt_at<?",
            [now - 300],
        )?;
        let records = {
            let mut statement = transaction.prepare("SELECT id,event_key,event_type,project,title,subtitle,body,status,attempts,next_attempt_at,created_at,sent_at,error,payload_json FROM events WHERE status IN ('queued','retrying') AND next_attempt_at<=? ORDER BY created_at LIMIT ?")?;
            let values = statement
                .query_map(params![now, limit], map_record)?
                .collect::<Result<Vec<_>, _>>()?;
            values
        };
        for record in &records {
            transaction.execute(
                "UPDATE events SET status='sending', next_attempt_at=? WHERE id=?",
                params![now, record.id],
            )?;
        }
        transaction.commit()?;
        Ok(records)
    }

    /// Returns the number of seconds until queue processing is useful. This lets the desktop
    /// worker stay asleep while the queue is empty instead of polling the keyring and network.
    pub fn next_due_delay(&self) -> CoreResult<Option<u64>> {
        let now = Utc::now().timestamp();
        let due_at: Option<i64> = self.connect()?.query_row(
            "SELECT MIN(CASE WHEN status='sending' THEN next_attempt_at + 300 ELSE next_attempt_at END) FROM events WHERE status IN ('queued','retrying','sending')",
            [],
            |row| row.get(0),
        )?;
        Ok(due_at.map(|timestamp| timestamp.saturating_sub(now).max(0) as u64))
    }

    pub fn mark_sent(&self, id: i64) -> CoreResult<()> {
        self.connect()?.execute(
            "UPDATE events SET status='sent',attempts=attempts+1,sent_at=?,error='' WHERE id=?",
            params![Utc::now().timestamp(), id],
        )?;
        Ok(())
    }
    pub fn mark_failed(&self, id: i64, error: &str, retry_limit: u32) -> CoreResult<()> {
        let delays = [10, 30, 120, 600, 1800, 3600, 7200, 14400];
        let connection = self.connect()?;
        let previous: u32 = connection
            .query_row("SELECT attempts FROM events WHERE id=?", [id], |row| {
                row.get(0)
            })
            .optional()?
            .unwrap_or(0);
        let attempts = previous + 1;
        let status = if attempts >= retry_limit {
            "failed"
        } else {
            "retrying"
        };
        let delay = delays[((attempts - 1) as usize).min(delays.len() - 1)];
        let safe_error: String = error.chars().take(500).collect();
        connection.execute(
            "UPDATE events SET status=?,attempts=?,next_attempt_at=?,error=? WHERE id=?",
            params![
                status,
                attempts,
                Utc::now().timestamp() + delay,
                safe_error,
                id
            ],
        )?;
        Ok(())
    }

    pub fn list(&self, limit: u32, status: Option<EventStatus>) -> CoreResult<Vec<EventRecord>> {
        let connection = self.connect()?;
        let sql = if status.is_some() {
            "SELECT id,event_key,event_type,project,title,subtitle,body,status,attempts,next_attempt_at,created_at,sent_at,error,payload_json FROM events WHERE status=? ORDER BY created_at DESC LIMIT ?"
        } else {
            "SELECT id,event_key,event_type,project,title,subtitle,body,status,attempts,next_attempt_at,created_at,sent_at,error,payload_json FROM events ORDER BY created_at DESC LIMIT ?"
        };
        let mut statement = connection.prepare(sql)?;
        let rows = if let Some(status) = status {
            statement
                .query_map(params![status.as_str(), limit.min(500)], map_record)?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            statement
                .query_map([limit.min(500)], map_record)?
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    pub fn get(&self, id: i64) -> CoreResult<Option<EventRecord>> {
        self.connect()?
            .query_row(
                "SELECT id,event_key,event_type,project,title,subtitle,body,status,attempts,next_attempt_at,created_at,sent_at,error,payload_json FROM events WHERE id=?",
                [id],
                map_record,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn retry(&self, id: Option<i64>) -> CoreResult<usize> {
        let now = Utc::now().timestamp();
        let c = self.connect()?;
        Ok(if let Some(id) = id {
            c.execute("UPDATE events SET status='queued',next_attempt_at=?,error='' WHERE id=? AND status='failed'",params![now,id])?
        } else {
            c.execute("UPDATE events SET status='queued',next_attempt_at=?,error='' WHERE status='failed'",[now])?
        })
    }
    pub fn clear_history(&self) -> CoreResult<usize> {
        Ok(self.connect()?.execute(
            "DELETE FROM events WHERE status IN ('sent','failed','suppressed')",
            [],
        )?)
    }
    pub fn counts(&self) -> CoreResult<EventCounts> {
        let c = self.connect()?;
        let mut s = c.prepare("SELECT status,COUNT(*) FROM events GROUP BY status")?;
        let map: HashMap<String, u32> = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        Ok(EventCounts {
            queued: *map.get("queued").unwrap_or(&0),
            sending: *map.get("sending").unwrap_or(&0),
            retrying: *map.get("retrying").unwrap_or(&0),
            sent: *map.get("sent").unwrap_or(&0),
            failed: *map.get("failed").unwrap_or(&0),
            suppressed: *map.get("suppressed").unwrap_or(&0),
        })
    }
}

fn map_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRecord> {
    Ok(EventRecord {
        id: row.get(0)?,
        event_key: row.get(1)?,
        event_type: row.get(2)?,
        project: row.get(3)?,
        title: row.get(4)?,
        subtitle: row.get(5)?,
        body: row.get(6)?,
        status: EventStatus::parse(&row.get::<_, String>(7)?),
        attempts: row.get(8)?,
        next_attempt_at: row.get(9)?,
        created_at: row.get(10)?,
        sent_at: row.get(11)?,
        error: row.get(12)?,
        payload_json: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_notification, AppSettings, HookEvent};
    #[test]
    fn deduplicates_and_retries() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path());
        let store = EventStore::new(&paths).unwrap();
        assert_eq!(store.next_due_delay().unwrap(), None);
        let event:HookEvent=serde_json::from_value(serde_json::json!({"hookEventName":"Stop","sessionId":"s","turnId":"t","cwd":"/tmp/demo"})).unwrap();
        let n = build_notification(&event, &AppSettings::default());
        assert!(store.enqueue(&n).unwrap().1);
        assert_eq!(store.next_due_delay().unwrap(), Some(0));
        assert!(!store.enqueue(&n).unwrap().1);
        let claimed = store.claim_due(5).unwrap();
        assert_eq!(claimed.len(), 1);
        store.mark_failed(claimed[0].id, "offline", 2).unwrap();
        assert_eq!(store.counts().unwrap().retrying, 1);
        assert!(matches!(store.next_due_delay().unwrap(), Some(1..=10)));
    }
}
