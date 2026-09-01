from __future__ import annotations

import ctypes
import os

_INSTANCE_MUTEX = None


def _acquire_single_instance() -> bool:
    """Keep one tray process and surface the existing window on a second launch."""
    global _INSTANCE_MUTEX
    if os.name != "nt":
        return True
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    handle = kernel32.CreateMutexW(None, False, "Local\\CodexBarkNotifier.Instance")
    if not handle:
        return True
    if ctypes.get_last_error() == 183:  # ERROR_ALREADY_EXISTS
        kernel32.CloseHandle(handle)
        user32 = ctypes.windll.user32
        window = user32.FindWindowW(None, "Codex Notify") or user32.FindWindowW(None, "Codex Bark Notifier")
        if window:
            user32.ShowWindow(window, 9)  # SW_RESTORE
            user32.SetForegroundWindow(window)
        return False
    _INSTANCE_MUTEX = handle
    return True


def _enable_windows_dpi_awareness() -> None:
    if os.name != "nt":
        return
    try:
        ctypes.windll.shcore.SetProcessDpiAwareness(2)
    except (AttributeError, OSError):
        try:
            ctypes.windll.user32.SetProcessDPIAware()
        except (AttributeError, OSError):
            pass


def main() -> None:
    if not _acquire_single_instance():
        return
    _enable_windows_dpi_awareness()
    from src.gui import NotifierApp

    app = NotifierApp()
    app.mainloop()


if __name__ == "__main__":
    main()
