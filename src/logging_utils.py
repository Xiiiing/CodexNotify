from __future__ import annotations

import logging
from logging.handlers import RotatingFileHandler

from .paths import LOG_FILE, ensure_runtime_dirs


def get_logger(name: str = "CodexBarkNotifier") -> logging.Logger:
    ensure_runtime_dirs()
    logger = logging.getLogger(name)
    if logger.handlers:
        return logger
    logger.setLevel(logging.INFO)
    handler = RotatingFileHandler(
        LOG_FILE,
        maxBytes=512 * 1024,
        backupCount=3,
        encoding="utf-8",
    )
    handler.setFormatter(logging.Formatter("%(asctime)s %(levelname)s %(message)s"))
    logger.addHandler(handler)
    return logger

