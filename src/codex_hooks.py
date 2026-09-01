from __future__ import annotations

import copy
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime
from pathlib import Path
from typing import Any

from .paths import PROJECT_ROOT


HOOK_MARKER = "--codex-bark-notifier"


class HookConfigError(RuntimeError):
    pass


def codex_home() -> Path:
    configured = os.environ.get("CODEX_HOME", "").strip()
    return Path(configured).expanduser().resolve() if configured else Path.home() / ".codex"


def hooks_path() -> Path:
    return codex_home() / "hooks.json"


def config_path() -> Path:
    return codex_home() / "config.toml"


def _load_document(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"description": "User lifecycle hooks for Codex.", "hooks": {}}
    try:
        value = json.loads(path.read_text(encoding="utf-8-sig"))
    except json.JSONDecodeError as exc:
        raise HookConfigError(
            f"{path} 不是合法 JSON（第 {exc.lineno} 行，第 {exc.colno} 列）。"
            "为防止覆盖现有配置，程序已停止修改。"
        ) from exc
    except OSError as exc:
        raise HookConfigError(f"无法读取 {path}：{exc}") from exc
    if not isinstance(value, dict):
        raise HookConfigError(f"{path} 的顶层必须是 JSON 对象。")
    hooks = value.setdefault("hooks", {})
    if not isinstance(hooks, dict):
        raise HookConfigError(f"{path} 中的 hooks 必须是 JSON 对象。")
    return value


def _is_ours(handler: Any) -> bool:
    if not isinstance(handler, dict):
        return False
    return HOOK_MARKER in str(handler.get("commandWindows", "")) or HOOK_MARKER in str(
        handler.get("command", "")
    )


def _remove_our_handlers(document: dict[str, Any]) -> int:
    removed = 0
    hooks = document.get("hooks", {})
    for event_name, groups in list(hooks.items()):
        if not isinstance(groups, list):
            continue
        retained_groups: list[Any] = []
        for group in groups:
            if not isinstance(group, dict) or not isinstance(group.get("hooks"), list):
                retained_groups.append(group)
                continue
            old_handlers = group["hooks"]
            new_handlers = [handler for handler in old_handlers if not _is_ours(handler)]
            removed += len(old_handlers) - len(new_handlers)
            if new_handlers:
                new_group = copy.deepcopy(group)
                new_group["hooks"] = new_handlers
                retained_groups.append(new_group)
        if retained_groups:
            hooks[event_name] = retained_groups
        else:
            hooks.pop(event_name, None)
    return removed


def _handler(event_name: str) -> dict[str, Any]:
    runner = (PROJECT_ROOT / "hook_runner.py").resolve()
    python = Path(sys.executable).resolve()
    windows_command = subprocess.list2cmdline([str(python), str(runner), HOOK_MARKER])
    generic_command = f'"{python}" "{runner}" {HOOK_MARKER}'
    return {
        "type": "command",
        "command": generic_command,
        "commandWindows": windows_command,
        "timeout": 30,
        "async": True,
        "statusMessage": "正在记录 Codex 通知",
    }


def _backup(path: Path) -> Path | None:
    if not path.exists():
        return None
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    backup = path.with_name(f"{path.name}.bak.{stamp}")
    counter = 1
    while backup.exists():
        backup = path.with_name(f"{path.name}.bak.{stamp}.{counter}")
        counter += 1
    shutil.copy2(path, backup)
    return backup


def _atomic_write(path: Path, document: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(document, ensure_ascii=False, indent=2) + "\n"
    handle, temp_name = tempfile.mkstemp(prefix="hooks-", suffix=".tmp", dir=path.parent)
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


def hook_status() -> dict[str, Any]:
    path = hooks_path()
    document = _load_document(path)
    count = 0
    current_handler = False
    current_interpreter = False
    configured_command = ""
    installed_events: set[str] = set()
    expected_runner = str((PROJECT_ROOT / "hook_runner.py").resolve()).casefold()
    expected_python = str(Path(sys.executable).resolve()).casefold()
    for event_name, groups in document.get("hooks", {}).items():
        if not isinstance(groups, list):
            continue
        for group in groups:
            if isinstance(group, dict) and isinstance(group.get("hooks"), list):
                for handler in group["hooks"]:
                    if _is_ours(handler):
                        count += 1
                        installed_events.add(str(event_name))
                        command = f"{handler.get('command', '')} {handler.get('commandWindows', '')}"
                        configured_command = command.strip()
                        if expected_runner in command.casefold():
                            current_handler = True
                        if expected_python in command.casefold():
                            current_interpreter = True

    disabled = False
    config = config_path()
    if config.exists():
        try:
            text = config.read_text(encoding="utf-8-sig")
            feature_match = re.search(
                r"(?ms)^\s*\[features\]\s*$\n(?P<body>.*?)(?=^\s*\[|\Z)", text
            )
            if feature_match and re.search(
                r"(?m)^\s*(?:hooks|codex_hooks)\s*=\s*false\s*(?:#.*)?$",
                feature_match.group("body"),
                re.IGNORECASE,
            ):
                disabled = True
        except OSError:
            pass

    return {
        "codex_home": str(codex_home()),
        "hooks_path": str(path),
        "exists": path.exists(),
        "installed": {"Stop", "PermissionRequest"}.issubset(installed_events),
        "handler_count": count,
        "installed_events": sorted(installed_events),
        "path_current": current_handler,
        "interpreter_current": current_interpreter,
        "configured_command": configured_command,
        "hooks_disabled": disabled,
        "python_executable": str(Path(sys.executable).resolve()),
        "python_exists": Path(sys.executable).exists(),
        "runner_exists": (PROJECT_ROOT / "hook_runner.py").exists(),
    }


def install_hook() -> tuple[Path, Path | None]:
    path = hooks_path()
    document = _load_document(path)
    _remove_our_handlers(document)
    hooks = document.setdefault("hooks", {})
    for event_name in ("Stop", "PermissionRequest"):
        groups = hooks.setdefault(event_name, [])
        if not isinstance(groups, list):
            raise HookConfigError(f"{path} 中 hooks.{event_name} 必须是数组。")
        groups.append({"hooks": [_handler(event_name)]})
    backup = _backup(path)
    _atomic_write(path, document)
    return path, backup


def uninstall_hook() -> tuple[Path, Path | None, int]:
    path = hooks_path()
    if not path.exists():
        return path, None, 0
    document = _load_document(path)
    removed = _remove_our_handlers(document)
    if not removed:
        return path, None, 0
    backup = _backup(path)
    _atomic_write(path, document)
    return path, backup, removed
