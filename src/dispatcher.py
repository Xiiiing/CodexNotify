from __future__ import annotations

import json
from typing import Any

from .bark_client import send_notification
from .event_store import claim_due, mark_failed, mark_sent
from .settings import load_bark_key, load_encryption_key, load_settings


def process_due_events(limit: int = 5) -> list[dict[str, Any]]:
    settings = load_settings()
    bark_key = load_bark_key()
    encryption_key = load_encryption_key() if settings.get("encryption_enabled") else ""
    results: list[dict[str, Any]] = []
    if not bark_key:
        return results
    for row in claim_due(limit):
        event_id = int(row["id"])
        notification = json.loads(str(row["payload_json"]))
        try:
            if settings.get("encryption_enabled") and not encryption_key:
                raise RuntimeError("已启用 Bark 加密，但未保存加密密钥。")
            send_notification(
                server=str(settings.get("bark_server") or "https://api.day.app"),
                key=bark_key,
                title=str(notification["title"]),
                subtitle=str(notification.get("subtitle") or ""),
                body=str(notification["body"]),
                group=str(notification.get("group") or "Codex"),
                level=str(notification.get("level") or "active"),
                sound=str(notification.get("sound") or ""),
                timeout=float(settings.get("request_timeout") or 8),
                icon=str(notification.get("icon") or ""),
                url=str(notification.get("url") or ""),
                encryption_key=encryption_key,
                encryption_algorithm=str(settings.get("encryption_algorithm") or "AES-128-CBC"),
            )
        except Exception as exc:
            mark_failed(event_id, str(exc), int(settings.get("retry_limit") or 5))
            results.append({"id": event_id, "sent": False, "error": str(exc)})
        else:
            mark_sent(event_id)
            results.append({"id": event_id, "sent": True, "error": ""})
    return results
