from __future__ import annotations

import json
import os
import tempfile
from datetime import datetime, timezone
from typing import Any

from .paths import DATA_DIR


HEALTH_FILE = DATA_DIR / "hook_health.json"


def record_real_hook_success(event: dict[str, Any], project: str) -> None:
    """Record only genuine Codex callbacks, never the bundled simulator."""
    session_id = str(event.get("session_id") or "")
    if session_id.startswith("manual-test") or session_id.startswith("environment-test"):
        return
    payload = {
        "last_success_at": datetime.now(timezone.utc).astimezone().isoformat(timespec="seconds"),
        "session_id": session_id,
        "turn_id": str(event.get("turn_id") or ""),
        "project": project,
        "cwd": str(event.get("cwd") or ""),
    }
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    handle, temporary = tempfile.mkstemp(prefix="health-", suffix=".tmp", dir=DATA_DIR)
    try:
        with os.fdopen(handle, "w", encoding="utf-8", newline="\n") as stream:
            json.dump(payload, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
        os.replace(temporary, HEALTH_FILE)
    except Exception:
        try:
            os.unlink(temporary)
        except OSError:
            pass
        raise


def load_hook_health() -> dict[str, Any]:
    if not HEALTH_FILE.exists():
        return {}
    try:
        value = json.loads(HEALTH_FILE.read_text(encoding="utf-8"))
        return value if isinstance(value, dict) else {}
    except (OSError, json.JSONDecodeError):
        return {}
