from __future__ import annotations

import re
from typing import Any


WAIT_PATTERNS = [
    r"需要你(?:确认|选择|提供|输入|授权)",
    r"请(?:确认|选择|提供|输入|告诉我)",
    r"等待(?:你的|用户)",
    r"need you to|please (?:confirm|choose|provide)|waiting for",
]
FAIL_PATTERNS = [
    r"任务失败",
    r"无法完成",
    r"执行失败",
    r"被阻止",
    r"failed|unable to complete|blocked",
]


def classify_stop(event: dict[str, Any]) -> tuple[str, str]:
    message = str(event.get("last_assistant_message") or "")
    if any(re.search(pattern, message, re.IGNORECASE) for pattern in FAIL_PATTERNS):
        return "执行异常", "⚠️"
    if any(re.search(pattern, message, re.IGNORECASE) for pattern in WAIT_PATTERNS):
        return "等待输入", "❓"
    return "任务回合结束", "✅"


def render_body(message: str, settings: dict[str, Any], status: str) -> str:
    mode = settings.get("message_mode", "summary_200")
    if mode == "minimal":
        return f"Codex 状态：{status}。"
    if mode == "fixed":
        return str(settings.get("fixed_message") or "Codex 已结束一轮任务。")

    compact = " ".join((message or "").strip().split())
    if not compact:
        compact = f"Codex 状态：{status}。请回到电脑查看结果。"
    if mode == "full":
        return compact
    limit = 500 if mode == "summary_500" else 200
    return compact if len(compact) <= limit else compact[: limit - 1] + "…"
