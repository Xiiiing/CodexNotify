from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from .event_classifier import classify_stop, render_body
from .privacy import redact_sensitive_text
from .project_filter import project_display_name
from .quiet_hours import is_quiet_now


class _SafeValues(dict[str, str]):
    def __missing__(self, key: str) -> str:
        return "{" + key + "}"


@dataclass
class Notification:
    event_key: str
    event_type: str
    session_id: str
    turn_id: str
    project: str
    cwd: str
    title: str
    subtitle: str
    body: str
    group: str
    level: str
    sound: str
    icon: str
    url: str
    suppressed: bool = False
    suppress_reason: str = ""

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


def _permission_body(event: dict[str, Any], settings: dict[str, Any]) -> str:
    tool_name = str(event.get("tool_name") or "本地操作")
    tool_input = event.get("tool_input")
    description = ""
    if isinstance(tool_input, dict):
        description = str(tool_input.get("description") or "")
        if not description:
            command = str(tool_input.get("command") or "")
            description = command[:240]
    if settings.get("message_mode") == "minimal":
        return f"Codex 请求批准：{tool_name}。"
    return f"请求：{tool_name}" + (f"\n{description}" if description else "\n请回到电脑审查后决定。")


def _event_key(event: dict[str, Any]) -> str:
    event_name = str(event.get("hook_event_name") or "unknown")
    session = str(event.get("session_id") or "")
    turn = str(event.get("turn_id") or "")
    extra = ""
    if event_name == "PermissionRequest":
        extra = str(event.get("tool_use_id") or "")
        if not extra:
            extra = json.dumps(
                {"tool_name": event.get("tool_name"), "tool_input": event.get("tool_input")},
                sort_keys=True,
                ensure_ascii=False,
                default=str,
            )
    raw = f"{event_name}|{session}|{turn}|{extra}".encode("utf-8", errors="replace")
    return hashlib.sha256(raw).hexdigest()


def build_notification(event: dict[str, Any], settings: dict[str, Any]) -> Notification:
    event_type = str(event.get("hook_event_name") or "")
    cwd = str(event.get("cwd") or "")
    project = project_display_name(cwd, settings)
    if event_type == "PermissionRequest":
        status, icon = "等待批准", "🔐"
        body = _permission_body(event, settings)
    else:
        status, icon = classify_stop(event)
        body = render_body(str(event.get("last_assistant_message") or ""), settings, status)

    if settings.get("redact_sensitive", True):
        body = redact_sensitive_text(body)
    values = _SafeValues(project=project, status=status, icon=icon)
    template = str(settings.get("notification_title") or "{project}")
    try:
        rendered_title = template.format_map(values)
    except (ValueError, AttributeError):
        rendered_title = project
    url_template = str(settings.get("click_url") or "")
    try:
        click_url = url_template.format_map(values)
    except (ValueError, AttributeError):
        click_url = url_template

    level = str(settings.get("level") or "active")
    sound = str(settings.get("sound") or "")
    suppressed = False
    suppress_reason = ""
    if is_quiet_now(settings):
        action = str(settings.get("quiet_action") or "silent")
        important = event_type == "PermissionRequest" or status == "执行异常"
        if action == "pause" or (action == "important_only" and not important):
            suppressed, suppress_reason = True, "安静时段已暂停此类通知"
        elif action == "silent":
            level, sound = "passive", ""

    return Notification(
        event_key=_event_key(event),
        event_type=event_type,
        session_id=str(event.get("session_id") or ""),
        turn_id=str(event.get("turn_id") or ""),
        project=project or Path(cwd).name or "未知项目",
        cwd=cwd,
        title=f"{icon} {rendered_title}",
        subtitle=status,
        body=body,
        group=str(settings.get("group") or "Codex"),
        level=level,
        sound=sound,
        icon=str(settings.get("bark_icon") or ""),
        url=click_url,
        suppressed=suppressed,
        suppress_reason=suppress_reason,
    )
