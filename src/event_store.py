from __future__ import annotations

import json
import sqlite3
import time
from typing import Any

from .notification_builder import Notification
from .paths import EVENTS_DB_FILE, ensure_runtime_dirs


SCHEMA = """
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_key TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    session_id TEXT,
    turn_id TEXT,
    project TEXT NOT NULL,
    title TEXT NOT NULL,
    subtitle TEXT,
    body TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    status TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    sent_at INTEGER,
    error TEXT DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_events_due ON events(status, next_attempt_at);
CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at DESC);
"""


def _connect() -> sqlite3.Connection:
    ensure_runtime_dirs()
    connection = sqlite3.connect(EVENTS_DB_FILE, timeout=10)
    connection.row_factory = sqlite3.Row
    connection.execute("PRAGMA journal_mode=WAL")
    connection.execute("PRAGMA busy_timeout=10000")
    connection.executescript(SCHEMA)
    return connection


def enqueue(notification: Notification) -> tuple[int, bool]:
    now = int(time.time())
    status = "suppressed" if notification.suppressed else "queued"
    with _connect() as connection:
        cursor = connection.execute(
            """INSERT OR IGNORE INTO events
            (event_key,event_type,session_id,turn_id,project,title,subtitle,body,payload_json,status,next_attempt_at,created_at,error)
            VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)""",
            (
                notification.event_key, notification.event_type, notification.session_id,
                notification.turn_id, notification.project, notification.title,
                notification.subtitle, notification.body,
                json.dumps(notification.to_dict(), ensure_ascii=False), status, now, now,
                notification.suppress_reason,
            ),
        )
        created = cursor.rowcount == 1
        row = connection.execute("SELECT id FROM events WHERE event_key=?", (notification.event_key,)).fetchone()
        return int(row["id"]), created


def claim_due(limit: int = 5) -> list[dict[str, Any]]:
    now = int(time.time())
    with _connect() as connection:
        connection.execute("BEGIN IMMEDIATE")
        connection.execute(
            "UPDATE events SET status='retrying' WHERE status='sending' AND next_attempt_at<?",
            (now - 300,),
        )
        rows = connection.execute(
            "SELECT * FROM events WHERE status IN ('queued','retrying') AND next_attempt_at<=? ORDER BY created_at LIMIT ?",
            (now, limit),
        ).fetchall()
        ids = [int(row["id"]) for row in rows]
        if ids:
            placeholders = ",".join("?" for _ in ids)
            connection.execute(
                f"UPDATE events SET status='sending', next_attempt_at=? WHERE id IN ({placeholders})",
                (now, *ids),
            )
        return [dict(row) for row in rows]


def mark_sent(event_id: int) -> None:
    now = int(time.time())
    with _connect() as connection:
        connection.execute(
            "UPDATE events SET status='sent', attempts=attempts+1, sent_at=?, error='' WHERE id=?",
            (now, event_id),
        )


def mark_failed(event_id: int, error: str, retry_limit: int) -> None:
    delays = (10, 30, 120, 600, 1800, 3600, 7200, 14400)
    now = int(time.time())
    with _connect() as connection:
        row = connection.execute("SELECT attempts FROM events WHERE id=?", (event_id,)).fetchone()
        attempts = int(row["attempts"] if row else 0) + 1
        final = attempts >= retry_limit
        delay = delays[min(attempts - 1, len(delays) - 1)]
        connection.execute(
            "UPDATE events SET status=?, attempts=?, next_attempt_at=?, error=? WHERE id=?",
            ("failed" if final else "retrying", attempts, now + delay, error[:500], event_id),
        )


def history(limit: int = 100) -> list[dict[str, Any]]:
    with _connect() as connection:
        return [dict(row) for row in connection.execute("SELECT * FROM events ORDER BY created_at DESC LIMIT ?", (limit,)).fetchall()]


def retry_failed(event_id: int | None = None) -> int:
    now = int(time.time())
    with _connect() as connection:
        if event_id is None:
            cursor = connection.execute("UPDATE events SET status='queued', next_attempt_at=?, error='' WHERE status='failed'", (now,))
        else:
            cursor = connection.execute("UPDATE events SET status='queued', next_attempt_at=?, error='' WHERE id=? AND status='failed'", (now, event_id))
        return cursor.rowcount


def counts() -> dict[str, int]:
    with _connect() as connection:
        rows = connection.execute("SELECT status, COUNT(*) AS total FROM events GROUP BY status").fetchall()
    return {str(row["status"]): int(row["total"]) for row in rows}


def clear_history() -> int:
    with _connect() as connection:
        cursor = connection.execute("DELETE FROM events WHERE status IN ('sent','failed','suppressed')")
        return cursor.rowcount
