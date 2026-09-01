from __future__ import annotations

import json
import os
import tempfile
import unittest
import urllib.parse
import base64
from pathlib import Path
from unittest.mock import patch

from src.bark_client import BarkError, _encrypted_form, _json_bytes, build_endpoint
from src.event_store import enqueue, history
from src.codex_hooks import HOOK_MARKER, hook_status, install_hook, uninstall_hook
from src.event_classifier import classify_stop, render_body
from src.notification_builder import build_notification
from src.privacy import redact_sensitive_text
from src.quiet_hours import is_quiet_now
from src.project_filter import project_display_name, should_notify
from hook_runner import parse_hook_input


class BarkClientTests(unittest.TestCase):
    def test_build_endpoint_from_key(self) -> None:
        self.assertEqual(build_endpoint("https://api.day.app/", "abc"), "https://api.day.app/abc")

    def test_build_endpoint_from_full_url(self) -> None:
        self.assertEqual(
            build_endpoint("https://api.day.app", "https://example.test/key"),
            "https://example.test/key",
        )

    def test_empty_key_is_rejected(self) -> None:
        with self.assertRaises(BarkError):
            build_endpoint("https://api.day.app", "")

    def test_lone_surrogate_is_replaced_during_json_encoding(self) -> None:
        encoded = _json_bytes({"body": "包含异常字符：\udca1，其余中文保留"})
        decoded = encoded.decode("utf-8")
        self.assertIn("其余中文保留", decoded)
        self.assertNotIn("\udca1", decoded)

    def test_aes_encrypted_form_round_trip(self) -> None:
        from cryptography.hazmat.primitives import padding
        from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
        data, content_type = _encrypted_form({"title": "画图", "body": "完成"}, "1234567890123456", "AES-128-CBC")
        values = urllib.parse.parse_qs(data.decode("ascii"))
        iv = values["iv"][0].encode("ascii")
        ciphertext = base64.b64decode(values["ciphertext"][0])
        decryptor = Cipher(algorithms.AES(b"1234567890123456"), modes.CBC(iv)).decryptor()
        padded = decryptor.update(ciphertext) + decryptor.finalize()
        unpadder = padding.PKCS7(128).unpadder()
        decoded = json.loads((unpadder.update(padded) + unpadder.finalize()).decode("utf-8"))
        self.assertEqual(decoded["title"], "画图")
        self.assertEqual(content_type, "application/x-www-form-urlencoded")


class ProjectFilterTests(unittest.TestCase):
    def setUp(self) -> None:
        self.settings = {
            "enabled": True,
            "scope": "include",
            "projects": [{"path": r"D:\code\demo", "name": "演示项目", "enabled": True}],
        }

    def test_include_child_path(self) -> None:
        self.assertTrue(should_notify(r"D:\code\demo\src", self.settings))

    def test_exclude_other_path(self) -> None:
        self.assertFalse(should_notify(r"D:\code\other", self.settings))

    def test_display_name(self) -> None:
        self.assertEqual(project_display_name(r"D:\code\demo\src", self.settings), "演示项目")


class EventClassifierTests(unittest.TestCase):
    def test_waiting_for_user(self) -> None:
        status, _ = classify_stop({"last_assistant_message": "请确认是否继续。"})
        self.assertEqual(status, "等待输入")

    def test_failure(self) -> None:
        status, _ = classify_stop({"last_assistant_message": "任务失败，无法完成。"})
        self.assertEqual(status, "执行异常")

    def test_message_truncation(self) -> None:
        body = render_body("a" * 250, {"message_mode": "summary_200"}, "任务回合结束")
        self.assertEqual(len(body), 200)
        self.assertTrue(body.endswith("…"))


class HookInputTests(unittest.TestCase):
    def test_utf8_chinese_is_preserved_from_binary_stdin(self) -> None:
        original = {
            "hook_event_name": "Stop",
            "cwd": r"C:\Users\31908\Desktop\画图",
            "last_assistant_message": "中文通知：任务已经完成。",
        }
        raw = json.dumps(original, ensure_ascii=False).encode("utf-8")
        parsed = parse_hook_input(raw)
        self.assertEqual(parsed, original)

    def test_utf8_bom_is_accepted(self) -> None:
        raw = b"\xef\xbb\xbf" + json.dumps({"hook_event_name": "Stop"}).encode("utf-8")
        self.assertEqual(parse_hook_input(raw)["hook_event_name"], "Stop")


class HookMergeTests(unittest.TestCase):
    def test_install_is_idempotent_and_uninstall_preserves_other_hooks(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            codex_home = Path(temp_dir)
            original = {
                "description": "existing",
                "hooks": {
                    "Stop": [
                        {
                            "hooks": [
                                {
                                    "type": "command",
                                    "command": "python existing.py",
                                    "timeout": 5,
                                }
                            ]
                        }
                    ]
                },
            }
            (codex_home / "hooks.json").write_text(
                json.dumps(original), encoding="utf-8"
            )
            with patch.dict(os.environ, {"CODEX_HOME": str(codex_home)}):
                install_hook()
                install_hook()
                status = hook_status()
                self.assertEqual(status["handler_count"], 2)
                self.assertEqual(status["installed_events"], ["PermissionRequest", "Stop"])
                self.assertTrue(status["path_current"])
                self.assertTrue(status["interpreter_current"])

                document = json.loads((codex_home / "hooks.json").read_text(encoding="utf-8"))
                encoded = json.dumps(document)
                self.assertIn(HOOK_MARKER, encoded)
                self.assertIn("python existing.py", encoded)

                _, _, removed = uninstall_hook()
                self.assertEqual(removed, 2)
                final = json.loads((codex_home / "hooks.json").read_text(encoding="utf-8"))
                final_encoded = json.dumps(final)
                self.assertNotIn(HOOK_MARKER, final_encoded)
                self.assertIn("python existing.py", final_encoded)


class PrivacyAndNotificationTests(unittest.TestCase):
    def test_redacts_common_secrets(self) -> None:
        value = redact_sensitive_text("api_key=abcdef123456 and user@example.com")
        self.assertNotIn("abcdef123456", value)
        self.assertNotIn("user@example.com", value)

    def test_permission_request_builds_approval_notification(self) -> None:
        notification = build_notification(
            {
                "hook_event_name": "PermissionRequest",
                "session_id": "s1",
                "turn_id": "t1",
                "cwd": r"C:\code\demo",
                "tool_name": "Bash",
                "tool_input": {"description": "运行测试"},
            },
            {"group": "Codex", "notification_title": "{project}", "message_mode": "summary_200"},
        )
        self.assertEqual(notification.subtitle, "等待批准")
        self.assertIn("Bash", notification.body)

    def test_quiet_hours_cross_midnight(self) -> None:
        from datetime import datetime
        settings = {"quiet_hours_enabled": True, "quiet_start": "22:00", "quiet_end": "08:00"}
        self.assertTrue(is_quiet_now(settings, datetime(2026, 1, 1, 23, 0)))
        self.assertTrue(is_quiet_now(settings, datetime(2026, 1, 1, 7, 0)))
        self.assertFalse(is_quiet_now(settings, datetime(2026, 1, 1, 12, 0)))

    def test_event_store_deduplicates_same_turn(self) -> None:
        import src.event_store as store
        with tempfile.TemporaryDirectory() as temp_dir, patch.object(store, "EVENTS_DB_FILE", Path(temp_dir) / "events.sqlite"):
            notification = build_notification(
                {"hook_event_name": "Stop", "session_id": "s1", "turn_id": "t1", "cwd": r"C:\code\demo", "last_assistant_message": "完成"},
                {"group": "Codex", "notification_title": "{project}", "message_mode": "summary_200"},
            )
            first_id, first_created = enqueue(notification)
            second_id, second_created = enqueue(notification)
            self.assertTrue(first_created)
            self.assertFalse(second_created)
            self.assertEqual(first_id, second_id)
            self.assertEqual(len(history()), 1)


if __name__ == "__main__":
    unittest.main()
