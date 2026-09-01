from __future__ import annotations

import os
from pathlib import Path
from typing import Any


def _normalized(path: str) -> str:
    return os.path.normcase(os.path.abspath(os.path.expandvars(os.path.expanduser(path))))


def _is_within(candidate: str, root: str) -> bool:
    try:
        return os.path.commonpath([_normalized(candidate), _normalized(root)]) == _normalized(root)
    except (ValueError, OSError):
        return False


def enabled_project_paths(settings: dict[str, Any]) -> list[str]:
    result: list[str] = []
    for item in settings.get("projects", []):
        if not isinstance(item, dict) or not item.get("enabled", True):
            continue
        path = str(item.get("path", "")).strip()
        if path:
            result.append(path)
    return result


def should_notify(cwd: str, settings: dict[str, Any]) -> bool:
    if not settings.get("enabled", True):
        return False
    scope = settings.get("scope", "all")
    if scope == "all":
        return True
    matched = any(_is_within(cwd, root) for root in enabled_project_paths(settings))
    return matched if scope == "include" else not matched


def project_display_name(cwd: str, settings: dict[str, Any]) -> str:
    matches: list[tuple[int, str]] = []
    for item in settings.get("projects", []):
        if not isinstance(item, dict) or not item.get("enabled", True):
            continue
        root = str(item.get("path", "")).strip()
        if root and _is_within(cwd, root):
            label = str(item.get("name", "")).strip() or Path(root).name
            matches.append((len(_normalized(root)), label))
    if matches:
        return max(matches, key=lambda pair: pair[0])[1]
    return Path(cwd).name if cwd else "未知项目"

