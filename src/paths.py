from __future__ import annotations

from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parent.parent
DATA_DIR = PROJECT_ROOT / "data"
LOG_DIR = PROJECT_ROOT / "logs"
SETTINGS_FILE = DATA_DIR / "settings.json"
SECRET_FILE = DATA_DIR / "secret.dat"
ENCRYPTION_SECRET_FILE = DATA_DIR / "encryption_secret.dat"
EVENTS_DB_FILE = DATA_DIR / "events.sqlite"
LOG_FILE = LOG_DIR / "notifier.log"


def ensure_runtime_dirs() -> None:
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    LOG_DIR.mkdir(parents=True, exist_ok=True)
