from __future__ import annotations

import json
import sys
from typing import Any

from src.dispatcher import process_due_events
from src.event_store import enqueue
from src.health import record_real_hook_success
from src.logging_utils import get_logger
from src.notification_builder import build_notification
from src.project_filter import should_notify
from src.settings import load_settings


def parse_hook_input(raw: bytes) -> dict[str, Any]:
    """Parse Codex hook JSON without relying on the Windows console code page."""
    try:
        text = raw.decode("utf-8-sig")
    except UnicodeDecodeError:
        # Preserve a usable notification if a future producer supplies malformed UTF-8.
        text = raw.decode("utf-8", errors="replace")

    event = json.loads(text)
    if not isinstance(event, dict):
        raise ValueError("Hook 输入必须是 JSON 对象。")
    return event


def handle_event(event: dict[str, Any]) -> bool:
    logger = get_logger("CodexBarkNotifier.hook")
    event_name = str(event.get("hook_event_name") or "")
    if event_name not in {"Stop", "PermissionRequest"}:
        logger.info("忽略事件：%s", event_name or "未知")
        return False

    settings = load_settings()
    if event_name == "PermissionRequest" and not settings.get("permission_notifications", True):
        logger.info("审批通知已关闭")
        return False
    cwd = str(event.get("cwd") or "")
    if not should_notify(cwd, settings):
        logger.info("项目过滤器已忽略：%s", cwd)
        return False

    notification = build_notification(event, settings)
    event_id, created = enqueue(notification)
    results = process_due_events(limit=5) if created and not notification.suppressed else []
    record_real_hook_success(event, notification.project)
    result = next((item for item in results if item["id"] == event_id), None)
    logger.info(
        "事件已处理：event=%s project=%s id=%s created=%s sent=%s suppressed=%s",
        event_name, notification.project, event_id, created,
        result.get("sent") if result else None, notification.suppressed,
    )
    return True


def main() -> int:
    logger = get_logger("CodexBarkNotifier.hook")
    try:
        # Codex writes UTF-8 JSON bytes. Reading through sys.stdin on Windows can
        # incorrectly apply CP936/GBK and corrupt Chinese before json parses it.
        event = parse_hook_input(sys.stdin.buffer.read())
        handle_event(event)
    except Exception as exc:
        logger.exception("Hook 执行失败：%s", exc)
        print(f"Codex Bark notification failed: {exc}", file=sys.stderr)

    # Stop 要求合法 JSON；PermissionRequest 的空对象表示不做审批决定。
    print("{}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
