from __future__ import annotations

from typing import Callable

try:
    import pystray
    from PIL import Image, ImageDraw
except ImportError:  # The UI remains usable even before optional dependencies install.
    pystray = None
    Image = None
    ImageDraw = None


class TrayController:
    def __init__(
        self,
        *,
        show: Callable[[], None],
        quit_app: Callable[[], None],
        toggle_enabled: Callable[[], None],
        is_enabled: Callable[[], bool],
    ) -> None:
        self._show = show
        self._quit = quit_app
        self._toggle_enabled = toggle_enabled
        self._is_enabled = is_enabled
        self._enabled = bool(is_enabled())
        self.icon = None

    @property
    def available(self) -> bool:
        return pystray is not None

    def start(self) -> bool:
        if pystray is None or Image is None or ImageDraw is None:
            return False
        image = Image.new("RGBA", (64, 64), (18, 24, 38, 255))
        draw = ImageDraw.Draw(image)
        draw.rounded_rectangle((6, 6, 58, 58), radius=14, fill=(37, 99, 235, 255))
        draw.ellipse((17, 16, 47, 46), fill=(255, 255, 255, 255))
        draw.polygon([(26, 43), (20, 55), (38, 45)], fill=(255, 255, 255, 255))
        menu = pystray.Menu(
            pystray.MenuItem("打开 Codex Bark Notifier", lambda *_: self._show(), default=True),
            pystray.MenuItem(
                "启用通知",
                lambda *_: self._toggle_enabled(),
                checked=lambda _item: self._enabled,
            ),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem("退出程序", lambda *_: self._quit()),
        )
        self.icon = pystray.Icon("CodexBarkNotifier", image, "Codex Bark Notifier", menu)
        self.icon.run_detached()
        return True

    def refresh(self, enabled: bool | None = None) -> None:
        if enabled is not None:
            self._enabled = enabled
        if self.icon is not None:
            self.icon.update_menu()

    def stop(self) -> None:
        if self.icon is not None:
            self.icon.stop()
            self.icon = None
