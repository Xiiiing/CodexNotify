from __future__ import annotations

import json
import base64
import secrets
import string
import urllib.error
import urllib.parse
import urllib.request
from typing import Any


class BarkError(RuntimeError):
    pass


def _json_bytes(payload: dict[str, Any]) -> bytes:
    """Serialize notification JSON while tolerating lone UTF-16 surrogates.

    Some Windows clients can pass hook text containing surrogate-escaped bytes.
    Such values are valid Python strings but cannot be encoded as strict UTF-8.
    Replacing only the invalid code units keeps the rest of the notification usable.
    """
    return json.dumps(payload, ensure_ascii=False).encode("utf-8", errors="replace")


def build_endpoint(server: str, key: str) -> str:
    key = key.strip()
    server = server.strip().rstrip("/")
    if key.startswith(("https://", "http://")):
        endpoint = key.rstrip("/")
    else:
        if not server:
            server = "https://api.day.app"
        if not key:
            raise BarkError("请输入 Bark Key 或完整推送地址。")
        endpoint = f"{server}/{urllib.parse.quote(key, safe='')}"
    parsed = urllib.parse.urlparse(endpoint)
    if parsed.scheme not in {"http", "https"} or not parsed.netloc:
        raise BarkError("Bark 地址无效，必须是 http:// 或 https:// 地址。")
    return endpoint


def send_notification(
    *,
    server: str,
    key: str,
    title: str,
    body: str,
    subtitle: str = "",
    group: str = "Codex",
    level: str = "active",
    sound: str = "",
    timeout: float = 6.0,
    icon: str = "",
    url: str = "",
    encryption_key: str = "",
    encryption_algorithm: str = "AES-128-CBC",
) -> dict[str, Any]:
    endpoint = build_endpoint(server, key)
    payload: dict[str, Any] = {
        "title": title,
        "body": body,
        "group": group or "Codex",
        "level": level or "active",
    }
    if subtitle:
        payload["subtitle"] = subtitle
    if sound:
        payload["sound"] = sound
    if icon:
        payload["icon"] = icon
    if url:
        payload["url"] = url

    if encryption_key:
        data, content_type = _encrypted_form(payload, encryption_key, encryption_algorithm)
    else:
        data, content_type = _json_bytes(payload), "application/json; charset=utf-8"

    request = urllib.request.Request(
        endpoint,
        data=data,
        headers={
            "Content-Type": content_type,
            "User-Agent": "CodexBarkNotifier/0.3",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            raw = response.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise BarkError(f"Bark 返回 HTTP {exc.code}：{detail[:200]}") from exc
    except urllib.error.URLError as exc:
        raise BarkError(f"无法连接 Bark：{exc.reason}") from exc
    except TimeoutError as exc:
        raise BarkError("连接 Bark 超时。") from exc

    try:
        result = json.loads(raw) if raw else {}
    except json.JSONDecodeError:
        result = {"raw": raw}
    if isinstance(result, dict) and result.get("code") not in {None, 200}:
        raise BarkError(f"Bark 拒绝了请求：{result}")
    return result if isinstance(result, dict) else {"result": result}


def _encrypted_form(
    payload: dict[str, Any], key_text: str, algorithm: str
) -> tuple[bytes, str]:
    try:
        from cryptography.hazmat.primitives import padding
        from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
    except ImportError as exc:
        raise BarkError("启用 Bark 加密需要安装 cryptography。") from exc

    key = key_text.encode("utf-8")
    expected = 16 if algorithm == "AES-128-CBC" else 32
    if len(key) != expected:
        raise BarkError(f"{algorithm} 密钥必须正好是 {expected} 个 UTF-8 字节。")
    iv_text = "".join(secrets.choice(string.ascii_letters + string.digits) for _ in range(16))
    iv = iv_text.encode("ascii")
    padder = padding.PKCS7(128).padder()
    raw = _json_bytes(payload)
    padded = padder.update(raw) + padder.finalize()
    encryptor = Cipher(algorithms.AES(key), modes.CBC(iv)).encryptor()
    ciphertext = base64.b64encode(encryptor.update(padded) + encryptor.finalize()).decode("ascii")
    data = urllib.parse.urlencode({"ciphertext": ciphertext, "iv": iv_text}).encode("ascii")
    return data, "application/x-www-form-urlencoded"
