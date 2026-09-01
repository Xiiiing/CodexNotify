from __future__ import annotations

import copy
import json
import os
import tempfile
from pathlib import Path
from typing import Any

from .paths import ENCRYPTION_SECRET_FILE, SECRET_FILE, SETTINGS_FILE, ensure_runtime_dirs
from .security import protect_text, unprotect_text


DEFAULT_SETTINGS: dict[str, Any] = {
    "enabled": True,
    "bark_server": "https://api.day.app",
    "group": "Codex",
    "level": "active",
    "sound": "",
    "scope": "all",
    "projects": [],
    "message_mode": "summary_200",
    "fixed_message": "Codex 已结束一轮任务，请回到电脑查看结果。",
    "notification_title": "{project}",
    "permission_notifications": True,
    "redact_sensitive": True,
    "quiet_hours_enabled": False,
    "quiet_start": "22:00",
    "quiet_end": "08:00",
    "quiet_action": "silent",
    "bark_icon": "",
    "click_url": "",
    "request_timeout": 8,
    "retry_limit": 5,
    "encryption_enabled": False,
    "encryption_algorithm": "AES-128-CBC",
    "setup_completed": False,
    "startup_enabled": False,
}


def _merge_defaults(value: dict[str, Any]) -> dict[str, Any]:
    merged = copy.deepcopy(DEFAULT_SETTINGS)
    for key in DEFAULT_SETTINGS:
        if key in value:
            merged[key] = value[key]
    if merged["scope"] not in {"all", "include", "exclude"}:
        merged["scope"] = "all"
    if merged["message_mode"] not in {"minimal", "fixed", "summary_200", "summary_500", "full"}:
        merged["message_mode"] = "summary_200"
    if merged["level"] not in {"active", "timeSensitive", "passive", "critical"}:
        merged["level"] = "active"
    if not isinstance(merged["projects"], list):
        merged["projects"] = []
    if merged["quiet_action"] not in {"silent", "pause", "important_only"}:
        merged["quiet_action"] = "silent"
    if merged["encryption_algorithm"] not in {"AES-128-CBC", "AES-256-CBC"}:
        merged["encryption_algorithm"] = "AES-128-CBC"
    try:
        merged["request_timeout"] = max(2, min(30, int(merged["request_timeout"])))
        merged["retry_limit"] = max(1, min(8, int(merged["retry_limit"])))
    except (TypeError, ValueError):
        merged["request_timeout"], merged["retry_limit"] = 8, 5
    return merged


def load_settings() -> dict[str, Any]:
    ensure_runtime_dirs()
    if not SETTINGS_FILE.exists():
        return copy.deepcopy(DEFAULT_SETTINGS)
    try:
        value = json.loads(SETTINGS_FILE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise RuntimeError(f"无法读取设置文件 {SETTINGS_FILE}：{exc}") from exc
    if not isinstance(value, dict):
        raise RuntimeError(f"设置文件 {SETTINGS_FILE} 的顶层必须是 JSON 对象。")
    return _merge_defaults(value)


def _atomic_write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle, temp_name = tempfile.mkstemp(prefix=path.name, suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(handle, "w", encoding="utf-8", newline="\n") as stream:
            stream.write(text)
        os.replace(temp_name, path)
    except Exception:
        try:
            os.unlink(temp_name)
        except OSError:
            pass
        raise


def save_settings(settings: dict[str, Any]) -> None:
    ensure_runtime_dirs()
    clean = _merge_defaults(settings)
    _atomic_write(SETTINGS_FILE, json.dumps(clean, ensure_ascii=False, indent=2) + "\n")


def save_bark_key(key: str) -> None:
    ensure_runtime_dirs()
    encrypted = protect_text(key.strip())
    _atomic_write(SECRET_FILE, encrypted + "\n")


def load_bark_key() -> str:
    ensure_runtime_dirs()
    if not SECRET_FILE.exists():
        return ""
    encrypted = SECRET_FILE.read_text(encoding="utf-8").strip()
    return unprotect_text(encrypted)


def save_encryption_key(key: str) -> None:
    ensure_runtime_dirs()
    _atomic_write(ENCRYPTION_SECRET_FILE, protect_text(key.strip()) + "\n")


def load_encryption_key() -> str:
    ensure_runtime_dirs()
    if not ENCRYPTION_SECRET_FILE.exists():
        return ""
    return unprotect_text(ENCRYPTION_SECRET_FILE.read_text(encoding="utf-8").strip())
