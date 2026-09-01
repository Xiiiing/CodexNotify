from __future__ import annotations

from datetime import datetime


def _minutes(value: str) -> int:
    hour, minute = value.split(":", 1)
    return int(hour) * 60 + int(minute)


def is_quiet_now(settings: dict, now: datetime | None = None) -> bool:
    if not settings.get("quiet_hours_enabled", False):
        return False
    current = now or datetime.now()
    try:
        start = _minutes(str(settings.get("quiet_start", "22:00")))
        end = _minutes(str(settings.get("quiet_end", "08:00")))
    except (ValueError, TypeError):
        return False
    point = current.hour * 60 + current.minute
    return start <= point < end if start < end else point >= start or point < end
