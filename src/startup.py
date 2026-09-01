from __future__ import annotations

import os
from pathlib import Path

from .paths import PROJECT_ROOT


def startup_file() -> Path:
    appdata = Path(os.environ.get("APPDATA", Path.home() / "AppData" / "Roaming"))
    return appdata / "Microsoft" / "Windows" / "Start Menu" / "Programs" / "Startup" / "CodexBarkNotifier.cmd"


def startup_enabled() -> bool:
    return startup_file().exists()


def set_startup_enabled(enabled: bool) -> Path:
    path = startup_file()
    if enabled:
        path.parent.mkdir(parents=True, exist_ok=True)
        start = PROJECT_ROOT / "start.bat"
        path.write_text(f'@echo off\r\nstart "" "{start}"\r\n', encoding="mbcs")
    elif path.exists():
        path.unlink()
    return path
