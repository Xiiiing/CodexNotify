from __future__ import annotations

import base64
import ctypes
import os
from ctypes import wintypes


class SecretStorageError(RuntimeError):
    pass


class _DataBlob(ctypes.Structure):
    _fields_ = [("cbData", wintypes.DWORD), ("pbData", ctypes.POINTER(ctypes.c_ubyte))]


def _blob_from_bytes(value: bytes) -> tuple[_DataBlob, ctypes.Array]:
    buffer = ctypes.create_string_buffer(value)
    blob = _DataBlob(len(value), ctypes.cast(buffer, ctypes.POINTER(ctypes.c_ubyte)))
    return blob, buffer


def protect_text(value: str) -> str:
    """Encrypt text for the current Windows user with DPAPI."""
    if os.name != "nt":
        raise SecretStorageError("DPAPI 仅支持 Windows。")
    if not value:
        return ""

    raw = value.encode("utf-8")
    input_blob, input_buffer = _blob_from_bytes(raw)
    output_blob = _DataBlob()

    crypt32 = ctypes.windll.crypt32
    kernel32 = ctypes.windll.kernel32
    crypt32.CryptProtectData.argtypes = [
        ctypes.POINTER(_DataBlob),
        wintypes.LPCWSTR,
        ctypes.POINTER(_DataBlob),
        wintypes.LPVOID,
        wintypes.LPVOID,
        wintypes.DWORD,
        ctypes.POINTER(_DataBlob),
    ]
    crypt32.CryptProtectData.restype = wintypes.BOOL

    if not crypt32.CryptProtectData(
        ctypes.byref(input_blob),
        "CodexBarkNotifier",
        None,
        None,
        None,
        0,
        ctypes.byref(output_blob),
    ):
        raise SecretStorageError(f"DPAPI 加密失败，错误码：{ctypes.get_last_error()}")

    try:
        encrypted = ctypes.string_at(output_blob.pbData, output_blob.cbData)
        return base64.b64encode(encrypted).decode("ascii")
    finally:
        kernel32.LocalFree(output_blob.pbData)
        del input_buffer


def unprotect_text(value: str) -> str:
    """Decrypt text previously encrypted for the current Windows user."""
    if os.name != "nt":
        raise SecretStorageError("DPAPI 仅支持 Windows。")
    if not value:
        return ""

    try:
        encrypted = base64.b64decode(value, validate=True)
    except ValueError as exc:
        raise SecretStorageError("密钥文件格式无效。") from exc

    input_blob, input_buffer = _blob_from_bytes(encrypted)
    output_blob = _DataBlob()

    crypt32 = ctypes.windll.crypt32
    kernel32 = ctypes.windll.kernel32
    crypt32.CryptUnprotectData.argtypes = [
        ctypes.POINTER(_DataBlob),
        ctypes.POINTER(wintypes.LPWSTR),
        ctypes.POINTER(_DataBlob),
        wintypes.LPVOID,
        wintypes.LPVOID,
        wintypes.DWORD,
        ctypes.POINTER(_DataBlob),
    ]
    crypt32.CryptUnprotectData.restype = wintypes.BOOL

    if not crypt32.CryptUnprotectData(
        ctypes.byref(input_blob),
        None,
        None,
        None,
        None,
        0,
        ctypes.byref(output_blob),
    ):
        raise SecretStorageError(
            "无法解密 Bark Key。请确认当前 Windows 账户与保存密钥时相同。"
        )

    try:
        decrypted = ctypes.string_at(output_blob.pbData, output_blob.cbData)
        return decrypted.decode("utf-8")
    finally:
        kernel32.LocalFree(output_blob.pbData)
        del input_buffer

