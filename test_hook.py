from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


def main() -> int:
    project_root = Path(__file__).resolve().parent
    event = {
        "session_id": "manual-test",
        "turn_id": "manual-test-turn",
        # Use the Chinese-named workspace so the real test covers both title and body.
        "cwd": str(project_root.parent),
        "hook_event_name": "Stop",
        "stop_hook_active": False,
        "last_assistant_message": "模拟任务已经完成，Codex Notify 工作正常。",
    }
    input_bytes = json.dumps(event, ensure_ascii=False).encode("utf-8")
    result = subprocess.run(
        [sys.executable, str(project_root / "hook_runner.py"), "--codex-bark-notifier"],
        input=input_bytes,
        capture_output=True,
        check=False,
    )
    stdout = result.stdout.decode("utf-8", errors="replace").strip()
    stderr = result.stderr.decode("utf-8", errors="replace").strip()
    print("stdout:", stdout)
    if stderr:
        print("stderr:", stderr)
    print("exit code:", result.returncode)
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
