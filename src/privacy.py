from __future__ import annotations

import re


PATTERNS = (
    (re.compile(r"(?i)\b(?:sk|rk|pk)-[A-Za-z0-9_-]{12,}\b"), "[已隐藏 API Key]"),
    (re.compile(r"(?i)(?:token|api[_ -]?key|secret|password)\s*[:=]\s*[^\s,;]{6,}"), "敏感字段=[已隐藏]"),
    (re.compile(r"https?://[^\s?]+\?[^\s]+"), "[已隐藏含参数链接]"),
    (re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b"), "[已隐藏邮箱]"),
)


def redact_sensitive_text(text: str) -> str:
    result = text
    for pattern, replacement in PATTERNS:
        result = pattern.sub(replacement, result)
    return result
