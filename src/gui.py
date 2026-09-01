from __future__ import annotations

import json
import os
import subprocess
import sys
import threading
import secrets
import string
from datetime import datetime
from pathlib import Path
from tkinter import filedialog, messagebox, simpledialog

import customtkinter as ctk

from .bark_client import send_notification
from .codex_hooks import hook_status, install_hook, uninstall_hook
from .dispatcher import process_due_events
from .event_store import clear_history, counts, history, retry_failed
from .health import load_hook_health
from .paths import LOG_FILE, PROJECT_ROOT, ensure_runtime_dirs
from .settings import (
    DEFAULT_SETTINGS, load_bark_key, load_encryption_key, load_settings,
    save_bark_key, save_encryption_key, save_settings,
)
from .startup import set_startup_enabled, startup_enabled
from .tray import TrayController


ctk.set_appearance_mode("light")
ctk.set_default_color_theme("blue")

FONT = "Microsoft YaHei UI"
BG = "#F5F7FB"
SURFACE = "#FFFFFF"
NAV = "#101827"
NAV_HOVER = "#1E293B"
PRIMARY = "#2864DC"
PRIMARY_HOVER = "#1E4FB5"
TEXT = "#172033"
MUTED = "#64748B"
BORDER = "#E2E8F0"
GREEN = "#15803D"
GREEN_BG = "#DCFCE7"
AMBER = "#B45309"
AMBER_BG = "#FEF3C7"
RED = "#B91C1C"
RED_BG = "#FEE2E2"

SCOPE_LABELS = {"所有 Codex 项目": "all", "仅选择的项目": "include", "排除选择的项目": "exclude"}
SCOPE_VALUES = {value: label for label, value in SCOPE_LABELS.items()}
MESSAGE_LABELS = {"最小隐私（仅状态）": "minimal", "只发送固定消息": "fixed", "最终回复前 200 字": "summary_200", "最终回复前 500 字": "summary_500", "完整最终回复": "full"}
MESSAGE_VALUES = {value: label for label, value in MESSAGE_LABELS.items()}
LEVEL_LABELS = {"普通通知": "active", "时效性通知": "timeSensitive", "静默通知": "passive", "重要警报": "critical"}
LEVEL_VALUES = {value: label for label, value in LEVEL_LABELS.items()}
QUIET_LABELS = {"静默发送": "silent", "暂停普通通知": "important_only", "暂停所有通知": "pause"}
QUIET_VALUES = {value: label for label, value in QUIET_LABELS.items()}


class NotifierApp(ctk.CTk):
    def __init__(self) -> None:
        super().__init__()
        self.title("Codex Notify")
        self.geometry("1180x760")
        self.minsize(1020, 680)
        self.configure(fg_color=BG)
        ensure_runtime_dirs()

        try:
            self.settings = load_settings()
        except Exception as exc:
            messagebox.showerror("设置读取失败", str(exc))
            self.settings = DEFAULT_SETTINGS.copy()
        try:
            saved_key = load_bark_key()
        except Exception as exc:
            saved_key = ""
            messagebox.showwarning("密钥读取失败", str(exc))
        try:
            encryption_key = load_encryption_key()
        except Exception:
            encryption_key = ""

        title = str(self.settings.get("notification_title") or "{project}")
        self.server_var = ctk.StringVar(value=str(self.settings["bark_server"]))
        self.key_var = ctk.StringVar(value=saved_key)
        self.group_var = ctk.StringVar(value=str(self.settings["group"]))
        self.level_var = ctk.StringVar(value=LEVEL_VALUES.get(str(self.settings["level"]), "普通通知"))
        self.sound_var = ctk.StringVar(value=str(self.settings["sound"]))
        self.title_var = ctk.StringVar(value="{project}" if title == "Codex · {project}" else title)
        self.message_mode_var = ctk.StringVar(value=MESSAGE_VALUES.get(str(self.settings["message_mode"]), "最终回复前 200 字"))
        self.fixed_message_var = ctk.StringVar(value=str(self.settings["fixed_message"]))
        self.scope_var = ctk.StringVar(value=SCOPE_VALUES.get(str(self.settings["scope"]), "所有 Codex 项目"))
        self.enabled_var = ctk.BooleanVar(value=bool(self.settings["enabled"]))
        self.permission_var = ctk.BooleanVar(value=bool(self.settings.get("permission_notifications", True)))
        self.redact_var = ctk.BooleanVar(value=bool(self.settings.get("redact_sensitive", True)))
        self.quiet_enabled_var = ctk.BooleanVar(value=bool(self.settings.get("quiet_hours_enabled", False)))
        self.quiet_start_var = ctk.StringVar(value=str(self.settings.get("quiet_start", "22:00")))
        self.quiet_end_var = ctk.StringVar(value=str(self.settings.get("quiet_end", "08:00")))
        self.quiet_action_var = ctk.StringVar(value=QUIET_VALUES.get(str(self.settings.get("quiet_action", "silent")), "静默发送"))
        self.icon_var = ctk.StringVar(value=str(self.settings.get("bark_icon", "")))
        self.click_url_var = ctk.StringVar(value=str(self.settings.get("click_url", "")))
        self.timeout_var = ctk.StringVar(value=str(self.settings.get("request_timeout", 8)))
        self.retry_var = ctk.StringVar(value=str(self.settings.get("retry_limit", 5)))
        self.encryption_enabled_var = ctk.BooleanVar(value=bool(self.settings.get("encryption_enabled", False)))
        self.encryption_algorithm_var = ctk.StringVar(value=str(self.settings.get("encryption_algorithm", "AES-128-CBC")))
        self.encryption_key_var = ctk.StringVar(value=encryption_key)
        self.startup_var = ctk.BooleanVar(value=startup_enabled())
        self.status_var = ctk.StringVar(value="准备就绪")
        self.projects = [dict(item) for item in self.settings.get("projects", [])]
        self.pages: dict[str, ctk.CTkFrame] = {}
        self.nav_buttons: dict[str, ctk.CTkButton] = {}
        self._key_visible = False
        self._encryption_visible = False
        self._really_quit = False
        self._dispatching = False
        self._wizard = None

        self._build_shell()
        self._build_dashboard()
        self._build_projects()
        self._build_settings()
        self._build_history()
        self._build_advanced()
        self._build_diagnostics()
        self.show_page("dashboard")

        self.tray = TrayController(
            show=lambda: self.after(0, self.show_window),
            quit_app=lambda: self.after(0, self.quit_app),
            toggle_enabled=lambda: self.after(0, self.toggle_enabled_from_tray),
            is_enabled=lambda: self.enabled_var.get(),
        )
        if self.tray.start():
            self.protocol("WM_DELETE_WINDOW", self.hide_to_tray)
        else:
            self.protocol("WM_DELETE_WINDOW", self.quit_app)
            self.status_var.set("托盘依赖不可用，请通过 start.bat 启动")
        self.refresh_all()
        if not self.settings.get("setup_completed", False):
            self.after(700, self.open_setup_wizard)
        self.after(15000, self._periodic_refresh)

    def _build_shell(self) -> None:
        self.grid_columnconfigure(1, weight=1)
        self.grid_rowconfigure(1, weight=1)
        nav = ctk.CTkFrame(self, width=238, corner_radius=0, fg_color=NAV)
        nav.grid(row=0, column=0, rowspan=3, sticky="nsew")
        nav.grid_propagate(False)
        ctk.CTkLabel(nav, text="CODEX", text_color="#60A5FA", font=(FONT, 12, "bold"), anchor="w").pack(fill="x", padx=28, pady=(30, 0))
        ctk.CTkLabel(nav, text="Bark Notifier", text_color="white", font=(FONT, 23, "bold"), anchor="w").pack(fill="x", padx=28, pady=(0, 34))
        for key, label in (("dashboard", "⌂   概览"), ("projects", "▣   项目管理"), ("settings", "⚙   通知设置"), ("history", "◷   通知历史"), ("advanced", "◈   高级设置"), ("diagnostics", "✓   环境诊断")):
            button = ctk.CTkButton(nav, text=label, command=lambda name=key: self.show_page(name), height=46, corner_radius=10, fg_color="transparent", hover_color=NAV_HOVER, text_color="#CBD5E1", font=(FONT, 14), anchor="w")
            button.pack(fill="x", padx=16, pady=3)
            self.nav_buttons[key] = button
        ctk.CTkLabel(nav, text="关闭窗口后继续在托盘运行\n右击托盘图标可彻底退出", text_color="#718096", font=(FONT, 11), justify="left", anchor="w").pack(side="bottom", fill="x", padx=28, pady=26)

        header = ctk.CTkFrame(self, height=74, corner_radius=0, fg_color=SURFACE, border_width=1, border_color=BORDER)
        header.grid(row=0, column=1, sticky="nsew")
        header.grid_propagate(False)
        header.grid_columnconfigure(0, weight=1)
        self.page_title = ctk.CTkLabel(header, text="概览", text_color=TEXT, font=(FONT, 21, "bold"), anchor="w")
        self.page_title.grid(row=0, column=0, sticky="w", padx=30, pady=20)
        self.enable_switch = ctk.CTkSwitch(header, text="启用通知", variable=self.enabled_var, command=self._enabled_changed, progress_color=PRIMARY, button_color="white", button_hover_color="#F8FAFC", font=(FONT, 13))
        self.enable_switch.grid(row=0, column=1, padx=30)

        self.content = ctk.CTkFrame(self, corner_radius=0, fg_color=BG)
        self.content.grid(row=1, column=1, sticky="nsew")
        self.content.grid_columnconfigure(0, weight=1)
        self.content.grid_rowconfigure(0, weight=1)
        footer = ctk.CTkFrame(self, height=48, corner_radius=0, fg_color=SURFACE, border_width=1, border_color=BORDER)
        footer.grid(row=2, column=1, sticky="nsew")
        footer.grid_propagate(False)
        footer.grid_columnconfigure(0, weight=1)
        ctk.CTkLabel(footer, textvariable=self.status_var, text_color=MUTED, font=(FONT, 11), anchor="w").grid(row=0, column=0, sticky="w", padx=28, pady=13)
        ctk.CTkButton(footer, text="保存设置", width=110, height=32, corner_radius=8, command=self.save_all, fg_color=PRIMARY, hover_color=PRIMARY_HOVER, font=(FONT, 12, "bold")).grid(row=0, column=1, padx=20, pady=8)

    def _page(self, key: str) -> ctk.CTkFrame:
        page = ctk.CTkFrame(self.content, corner_radius=0, fg_color=BG)
        page.grid_columnconfigure(0, weight=1)
        self.pages[key] = page
        return page

    def _card(self, parent, title: str | None = None, **kwargs) -> ctk.CTkFrame:
        card = ctk.CTkFrame(parent, corner_radius=14, fg_color=SURFACE, border_width=1, border_color=BORDER, **kwargs)
        if title:
            ctk.CTkLabel(card, text=title, text_color=TEXT, font=(FONT, 15, "bold"), anchor="w").pack(fill="x", padx=22, pady=(19, 8))
        return card

    def _primary(self, parent, text: str, command, width: int = 120) -> ctk.CTkButton:
        return ctk.CTkButton(parent, text=text, command=command, width=width, height=38, corner_radius=9, fg_color=PRIMARY, hover_color=PRIMARY_HOVER, font=(FONT, 12, "bold"))

    def _secondary(self, parent, text: str, command, width: int = 100) -> ctk.CTkButton:
        return ctk.CTkButton(parent, text=text, command=command, width=width, height=36, corner_radius=9, fg_color="#EEF2F7", hover_color="#E2E8F0", text_color=TEXT, font=(FONT, 11))

    def _build_dashboard(self) -> None:
        page = self._page("dashboard")
        page.grid_rowconfigure(2, weight=1)
        hero = ctk.CTkFrame(page, height=150, corner_radius=16, fg_color=PRIMARY)
        hero.grid(row=0, column=0, sticky="ew", padx=28, pady=(26, 16))
        hero.grid_propagate(False)
        hero.grid_columnconfigure(0, weight=1)
        ctk.CTkLabel(hero, text="让 Codex 的每次完成，都准时抵达手腕", text_color="white", font=(FONT, 23, "bold"), anchor="w").grid(row=0, column=0, sticky="ew", padx=28, pady=(24, 0))
        self.hero_subtitle = ctk.CTkLabel(hero, text="", text_color="#DBEAFE", font=(FONT, 12), anchor="w")
        self.hero_subtitle.grid(row=1, column=0, sticky="ew", padx=28, pady=(3, 10))
        hero_actions = ctk.CTkFrame(hero, fg_color="transparent")
        hero_actions.grid(row=2, column=0, sticky="w", padx=28)
        ctk.CTkButton(hero_actions, text="发送测试通知", command=self.test_bark, width=130, height=36, corner_radius=9, fg_color="white", hover_color="#EFF6FF", text_color=PRIMARY, font=(FONT, 12, "bold")).pack(side="left")
        ctk.CTkButton(hero_actions, text="运行环境诊断", command=lambda: self.show_page("diagnostics"), width=130, height=36, corner_radius=9, fg_color="#3B75E5", hover_color="#4B83EA", text_color="white", font=(FONT, 12)).pack(side="left", padx=10)

        metrics = ctk.CTkFrame(page, fg_color="transparent")
        metrics.grid(row=1, column=0, sticky="ew", padx=28)
        metrics.grid_columnconfigure((0, 1, 2), weight=1, uniform="metric")
        self.metric_values: dict[str, ctk.CTkLabel] = {}
        for col, (key, title, icon) in enumerate((("project", "最近项目", "P"), ("hook", "Hook 状态", "H"), ("bark", "Bark 配置", "B"))):
            card = self._card(metrics)
            card.grid(row=0, column=col, sticky="ew", padx=(0 if col == 0 else 6, 0 if col == 2 else 6))
            badge = ctk.CTkLabel(card, text=icon, width=38, height=38, corner_radius=10, fg_color="#E8F0FF", text_color=PRIMARY, font=(FONT, 14, "bold"))
            badge.pack(side="left", padx=(20, 13), pady=20)
            text_box = ctk.CTkFrame(card, fg_color="transparent")
            text_box.pack(side="left", fill="both", expand=True, pady=16)
            ctk.CTkLabel(text_box, text=title, text_color=MUTED, font=(FONT, 11), anchor="w").pack(fill="x")
            value = ctk.CTkLabel(text_box, text="—", text_color=TEXT, font=(FONT, 16, "bold"), anchor="w")
            value.pack(fill="x", pady=(2, 0))
            self.metric_values[key] = value
        scope = self._card(page, "当前通知范围")
        scope.grid(row=2, column=0, sticky="nsew", padx=28, pady=(16, 24))
        self.project_summary = ctk.CTkLabel(scope, text="", text_color=TEXT, font=(FONT, 13), justify="left", anchor="nw", wraplength=760)
        self.project_summary.pack(fill="both", expand=True, padx=22, pady=(4, 20))

    def _build_projects(self) -> None:
        page = self._page("projects")
        page.grid_rowconfigure(1, weight=1)
        scope = self._card(page)
        scope.grid(row=0, column=0, sticky="ew", padx=28, pady=(26, 14))
        ctk.CTkLabel(scope, text="通知范围", text_color=TEXT, font=(FONT, 14, "bold")).pack(side="left", padx=(22, 12), pady=18)
        ctk.CTkOptionMenu(scope, variable=self.scope_var, values=list(SCOPE_LABELS), width=180, height=36, corner_radius=8, fg_color="#EEF2F7", button_color="#DDE5F0", button_hover_color="#CBD5E1", text_color=TEXT, dropdown_font=(FONT, 11), font=(FONT, 11)).pack(side="left")
        self._primary(scope, "＋ 添加项目", self.add_project, 120).pack(side="right", padx=20, pady=14)
        ctk.CTkLabel(scope, text="项目名称将直接作为 Bark 标题", text_color=PRIMARY, font=(FONT, 11, "bold")).pack(side="right", padx=14)
        self.project_list = ctk.CTkScrollableFrame(page, corner_radius=14, fg_color=SURFACE, border_width=1, border_color=BORDER, label_text="项目列表", label_font=(FONT, 15, "bold"), label_text_color=TEXT)
        self.project_list.grid(row=1, column=0, sticky="nsew", padx=28, pady=(0, 24))
        self.project_list.grid_columnconfigure(0, weight=1)

    def _labeled_entry(self, parent, row: int, label: str, variable, placeholder: str = "", show: str = "") -> ctk.CTkEntry:
        ctk.CTkLabel(parent, text=label, text_color=TEXT, font=(FONT, 12, "bold"), anchor="w").grid(row=row, column=0, sticky="w", padx=(22, 18), pady=10)
        entry = ctk.CTkEntry(parent, textvariable=variable, placeholder_text=placeholder, show=show, height=40, corner_radius=9, border_color="#CBD5E1", font=(FONT, 12))
        entry.grid(row=row, column=1, sticky="ew", padx=(0, 22), pady=10)
        return entry

    def _build_settings(self) -> None:
        page = self._page("settings")
        account_card = self._card(page, "Bark 账户")
        account_card.grid(row=0, column=0, sticky="ew", padx=28, pady=(26, 14))
        account = ctk.CTkFrame(account_card, fg_color="transparent")
        account.pack(fill="x", padx=0, pady=(0, 10))
        account.grid_columnconfigure(1, weight=1)
        self._labeled_entry(account, 0, "Bark 服务器", self.server_var)
        key_row = ctk.CTkFrame(account, fg_color="transparent")
        key_row.grid(row=1, column=1, sticky="ew", padx=(0, 22), pady=10)
        key_row.grid_columnconfigure(0, weight=1)
        ctk.CTkLabel(account, text="设备 Key", text_color=TEXT, font=(FONT, 12, "bold"), anchor="w").grid(row=1, column=0, sticky="w", padx=(22, 18))
        self.key_entry = ctk.CTkEntry(key_row, textvariable=self.key_var, show="●", height=40, corner_radius=9, border_color="#CBD5E1", font=(FONT, 12))
        self.key_entry.grid(row=0, column=0, sticky="ew")
        self._secondary(key_row, "显示", self.toggle_key, 70).grid(row=0, column=1, padx=(10, 0))
        ctk.CTkLabel(account, text="使用 Windows DPAPI 加密，仅当前 Windows 用户可解密。", text_color=MUTED, font=(FONT, 10), anchor="w").grid(row=2, column=1, sticky="w", padx=(0, 22), pady=(0, 12))

        style_card = self._card(page, "通知内容")
        style_card.grid(row=1, column=0, sticky="nsew", padx=28, pady=(0, 24))
        style = ctk.CTkFrame(style_card, fg_color="transparent")
        style.pack(fill="both", expand=True, padx=0, pady=(0, 4))
        style.grid_columnconfigure(1, weight=1)
        self._labeled_entry(style, 0, "标题模板", self.title_var)
        ctk.CTkLabel(style, text="推荐 {project}，这样项目名就是通知主标题。", text_color=PRIMARY, font=(FONT, 10), anchor="w").grid(row=1, column=1, sticky="w", pady=(0, 5))
        self._labeled_entry(style, 2, "通知分组", self.group_var)
        self._labeled_entry(style, 3, "通知声音", self.sound_var, "留空使用 Bark 默认声音")
        self._option_row(style, 4, "通知级别", self.level_var, list(LEVEL_LABELS))
        self._option_row(style, 5, "正文内容", self.message_mode_var, list(MESSAGE_LABELS))
        self._labeled_entry(style, 6, "固定消息", self.fixed_message_var)
        actions = ctk.CTkFrame(style, fg_color="transparent")
        actions.grid(row=7, column=1, sticky="w", padx=(0, 22), pady=(10, 22))
        self._primary(actions, "保存设置", self.save_all).pack(side="left")
        self._secondary(actions, "保存并测试", self.save_and_test, 120).pack(side="left", padx=10)
        preview = ctk.CTkFrame(style, width=260, corner_radius=14, fg_color="#F1F5F9", border_width=1, border_color="#CBD5E1")
        preview.grid(row=0, column=2, rowspan=8, sticky="nsew", padx=(0, 22), pady=(4, 20))
        preview.grid_propagate(False)
        ctk.CTkLabel(preview, text="通知实时预览", text_color=MUTED, font=(FONT, 10, "bold"), anchor="w").pack(fill="x", padx=18, pady=(16, 12))
        self.preview_title = ctk.CTkLabel(preview, text="✅ 示例项目", text_color=TEXT, font=(FONT, 16, "bold"), anchor="w", wraplength=220)
        self.preview_title.pack(fill="x", padx=18)
        self.preview_subtitle = ctk.CTkLabel(preview, text="任务回合结束", text_color=PRIMARY, font=(FONT, 11, "bold"), anchor="w")
        self.preview_subtitle.pack(fill="x", padx=18, pady=(4, 10))
        self.preview_body = ctk.CTkLabel(preview, text="任务已经完成，请查看结果。", text_color=MUTED, font=(FONT, 11), justify="left", anchor="nw", wraplength=220)
        self.preview_body.pack(fill="both", expand=True, padx=18, pady=(0, 16))
        for variable in (self.title_var, self.message_mode_var, self.fixed_message_var):
            variable.trace_add("write", lambda *_: self._update_preview())

    def _option_row(self, parent, row: int, label: str, variable, values: list[str]) -> None:
        ctk.CTkLabel(parent, text=label, text_color=TEXT, font=(FONT, 12, "bold"), anchor="w").grid(row=row, column=0, sticky="w", padx=(22, 18), pady=10)
        ctk.CTkOptionMenu(parent, variable=variable, values=values, height=40, corner_radius=9, fg_color="#EEF2F7", button_color="#DDE5F0", button_hover_color="#CBD5E1", text_color=TEXT, dropdown_font=(FONT, 11), font=(FONT, 11)).grid(row=row, column=1, sticky="ew", padx=(0, 22), pady=10)

    def _build_diagnostics(self) -> None:
        page = self._page("diagnostics")
        page.grid_rowconfigure(1, weight=1)
        actions = self._card(page)
        actions.grid(row=0, column=0, sticky="ew", padx=28, pady=(26, 14))
        self._primary(actions, "重新检查", self.refresh_all, 105).pack(side="left", padx=(20, 6), pady=15)
        for text, command, width in (("测试 Bark", self.test_bark, 100), ("模拟完整 Hook", self.test_hook_chain, 125), ("审核 / 信任 Hook", self.open_hook_review, 140), ("安装 / 修复", self.install_or_repair_hook, 110)):
            self._secondary(actions, text, command, width).pack(side="left", padx=5, pady=15)
        self._secondary(actions, "日志", self.open_log_dir, 70).pack(side="right", padx=20, pady=15)
        self._secondary(actions, "设置向导", self.open_setup_wizard, 90).pack(side="right", padx=0, pady=15)
        self.check_list = ctk.CTkScrollableFrame(page, corner_radius=14, fg_color=SURFACE, border_width=1, border_color=BORDER, label_text="环境健康检查", label_font=(FONT, 15, "bold"), label_text_color=TEXT)
        self.check_list.grid(row=1, column=0, sticky="nsew", padx=28, pady=(0, 14))
        self.check_list.grid_columnconfigure(0, weight=1)
        note = ctk.CTkFrame(page, corner_radius=11, fg_color="#FFFBEB", border_width=1, border_color="#FDE68A")
        note.grid(row=2, column=0, sticky="ew", padx=28, pady=(0, 24))
        ctk.CTkLabel(note, text="信任状态以真实 Codex Stop 回调为准。模拟测试只验证本地链路，不会冒充“已信任”。", text_color="#92400E", font=(FONT, 11), anchor="w", wraplength=820).pack(fill="x", padx=16, pady=12)

    def _build_history(self) -> None:
        page = self._page("history")
        page.grid_rowconfigure(1, weight=1)
        actions = self._card(page)
        actions.grid(row=0, column=0, sticky="ew", padx=28, pady=(26, 14))
        self.history_summary = ctk.CTkLabel(actions, text="", text_color=TEXT, font=(FONT, 13, "bold"), anchor="w")
        self.history_summary.pack(side="left", padx=20, pady=18)
        self._secondary(actions, "重试失败项", self.retry_all_failed, 110).pack(side="right", padx=(5, 20), pady=14)
        self._secondary(actions, "清理已完成", self.clear_finished_history, 110).pack(side="right", padx=5, pady=14)
        self.history_list = ctk.CTkScrollableFrame(page, corner_radius=14, fg_color=SURFACE, border_width=1, border_color=BORDER, label_text="最近事件", label_font=(FONT, 15, "bold"), label_text_color=TEXT)
        self.history_list.grid(row=1, column=0, sticky="nsew", padx=28, pady=(0, 24))
        self.history_list.grid_columnconfigure(0, weight=1)

    def _build_advanced(self) -> None:
        page = self._page("advanced")
        scroll = ctk.CTkScrollableFrame(page, corner_radius=0, fg_color=BG)
        scroll.grid(row=0, column=0, sticky="nsew")
        scroll.grid_columnconfigure(0, weight=1)
        behavior = self._card(scroll, "通知行为")
        behavior.grid(row=0, column=0, sticky="ew", padx=28, pady=(26, 14))
        ctk.CTkSwitch(behavior, text="审批请求立即通知（只提醒，不自动批准）", variable=self.permission_var, progress_color=PRIMARY, font=(FONT, 12)).pack(anchor="w", padx=22, pady=(10, 8))
        ctk.CTkSwitch(behavior, text="自动隐藏疑似 Token、邮箱和带参数链接", variable=self.redact_var, progress_color=PRIMARY, font=(FONT, 12)).pack(anchor="w", padx=22, pady=(0, 18))

        quiet_card = self._card(scroll, "安静时段")
        quiet_card.grid(row=1, column=0, sticky="ew", padx=28, pady=(0, 14))
        quiet = ctk.CTkFrame(quiet_card, fg_color="transparent")
        quiet.pack(fill="x", pady=(0, 8))
        quiet.grid_columnconfigure(1, weight=1)
        ctk.CTkSwitch(quiet, text="启用安静时段", variable=self.quiet_enabled_var, progress_color=PRIMARY, font=(FONT, 12)).grid(row=0, column=0, sticky="w", padx=22, pady=16)
        time_row = ctk.CTkFrame(quiet, fg_color="transparent")
        time_row.grid(row=0, column=1, sticky="e", padx=22)
        ctk.CTkEntry(time_row, textvariable=self.quiet_start_var, width=82, height=36, corner_radius=8, font=(FONT, 11)).pack(side="left")
        ctk.CTkLabel(time_row, text="至", text_color=MUTED, font=(FONT, 11)).pack(side="left", padx=8)
        ctk.CTkEntry(time_row, textvariable=self.quiet_end_var, width=82, height=36, corner_radius=8, font=(FONT, 11)).pack(side="left")
        ctk.CTkOptionMenu(quiet, variable=self.quiet_action_var, values=list(QUIET_LABELS), height=38, fg_color="#EEF2F7", button_color="#DDE5F0", text_color=TEXT, font=(FONT, 11)).grid(row=1, column=0, columnspan=2, sticky="ew", padx=22, pady=(0, 18))

        bark_card = self._card(scroll, "Bark 高级参数")
        bark_card.grid(row=2, column=0, sticky="ew", padx=28, pady=(0, 14))
        bark = ctk.CTkFrame(bark_card, fg_color="transparent")
        bark.pack(fill="x", pady=(0, 8))
        bark.grid_columnconfigure(1, weight=1)
        self._labeled_entry(bark, 0, "通知图标 URL", self.icon_var, "https://...")
        self._labeled_entry(bark, 1, "点击跳转 URL", self.click_url_var, "支持 {project}、{status}")
        self._labeled_entry(bark, 2, "请求超时（秒）", self.timeout_var)
        self._labeled_entry(bark, 3, "最大尝试次数", self.retry_var)

        encryption_card = self._card(scroll, "Bark 内容加密")
        encryption_card.grid(row=3, column=0, sticky="ew", padx=28, pady=(0, 14))
        encryption = ctk.CTkFrame(encryption_card, fg_color="transparent")
        encryption.pack(fill="x", pady=(0, 8))
        encryption.grid_columnconfigure(1, weight=1)
        ctk.CTkSwitch(encryption, text="启用端到端 AES-CBC 加密", variable=self.encryption_enabled_var, progress_color=PRIMARY, font=(FONT, 12)).grid(row=0, column=0, columnspan=2, sticky="w", padx=22, pady=14)
        self._option_row(encryption, 1, "加密算法", self.encryption_algorithm_var, ["AES-128-CBC", "AES-256-CBC"])
        self.encryption_key_entry = self._labeled_entry(encryption, 2, "加密密钥", self.encryption_key_var, "AES-128 为16字节；AES-256 为32字节", "●")
        encryption_buttons = ctk.CTkFrame(encryption, fg_color="transparent")
        encryption_buttons.grid(row=2, column=2, padx=(0, 22))
        self._secondary(encryption_buttons, "生成", self.generate_encryption_key, 64).pack(side="left")
        self._secondary(encryption_buttons, "显示", self.toggle_encryption_key, 64).pack(side="left", padx=(6, 0))
        ctk.CTkLabel(encryption, text="需要在 Bark App 的“推送加密”中设置相同算法和密钥。每条通知使用随机 IV。", text_color=MUTED, font=(FONT, 10), anchor="w").grid(row=3, column=1, sticky="w", padx=(0, 22), pady=(0, 16))

        system = self._card(scroll, "Windows")
        system.grid(row=4, column=0, sticky="ew", padx=28, pady=(0, 24))
        ctk.CTkSwitch(system, text="登录 Windows 后自动在托盘启动", variable=self.startup_var, progress_color=PRIMARY, font=(FONT, 12)).pack(anchor="w", padx=22, pady=16)
        self._primary(system, "保存高级设置", self.save_all, 130).pack(anchor="e", padx=22, pady=(0, 18))

    def show_page(self, key: str) -> None:
        titles = {"dashboard": "概览", "projects": "项目管理", "settings": "通知设置", "history": "通知历史", "advanced": "高级设置", "diagnostics": "环境诊断"}
        for page in self.pages.values():
            page.grid_forget()
        self.pages[key].grid(row=0, column=0, sticky="nsew")
        self.page_title.configure(text=titles[key])
        for name, button in self.nav_buttons.items():
            button.configure(fg_color=NAV_HOVER if name == key else "transparent", text_color="white" if name == key else "#CBD5E1")
        if key in {"dashboard", "history", "diagnostics"}:
            self.refresh_all()

    def _collect_settings(self) -> dict:
        return {
            "enabled": self.enabled_var.get(), "bark_server": self.server_var.get().strip() or "https://api.day.app",
            "group": self.group_var.get().strip() or "Codex", "level": LEVEL_LABELS[self.level_var.get()],
            "sound": self.sound_var.get().strip(), "scope": SCOPE_LABELS[self.scope_var.get()],
            "projects": self.projects, "message_mode": MESSAGE_LABELS[self.message_mode_var.get()],
            "fixed_message": self.fixed_message_var.get().strip(), "notification_title": self.title_var.get().strip() or "{project}",
            "permission_notifications": self.permission_var.get(), "redact_sensitive": self.redact_var.get(),
            "quiet_hours_enabled": self.quiet_enabled_var.get(), "quiet_start": self.quiet_start_var.get().strip(),
            "quiet_end": self.quiet_end_var.get().strip(), "quiet_action": QUIET_LABELS[self.quiet_action_var.get()],
            "bark_icon": self.icon_var.get().strip(), "click_url": self.click_url_var.get().strip(),
            "request_timeout": int(self.timeout_var.get()), "retry_limit": int(self.retry_var.get()),
            "encryption_enabled": self.encryption_enabled_var.get(), "encryption_algorithm": self.encryption_algorithm_var.get(),
            "setup_completed": bool(self.settings.get("setup_completed", False)), "startup_enabled": self.startup_var.get(),
        }

    def save_all(self, quiet: bool = False) -> bool:
        try:
            settings = self._collect_settings()
            self._validate_advanced(settings)
            save_settings(settings)
            save_bark_key(self.key_var.get())
            save_encryption_key(self.encryption_key_var.get())
            set_startup_enabled(self.startup_var.get())
            self.settings = settings
            self.status_var.set("设置已保存 · Bark Key 已加密")
            if hasattr(self, "tray"):
                self.tray.refresh(self.enabled_var.get())
            if not quiet:
                messagebox.showinfo("保存成功", "所有设置已保存。")
            self.refresh_all()
            return True
        except Exception as exc:
            messagebox.showerror("保存失败", str(exc))
            return False

    def save_and_test(self) -> None:
        if self.save_all(quiet=True):
            self.test_bark()

    def test_bark(self) -> None:
        if not self.key_var.get().strip():
            messagebox.showwarning("缺少 Bark Key", "请先填写 Bark Key。")
            self.show_page("settings")
            return
        try:
            settings = self._collect_settings()
            self._validate_advanced(settings)
        except Exception as exc:
            messagebox.showerror("设置无效", str(exc))
            return
        self.status_var.set("正在连接 Bark 服务器…")
        def worker() -> None:
            try:
                encryption_key = self.encryption_key_var.get().strip() if settings.get("encryption_enabled") else ""
                send_notification(server=settings["bark_server"], key=self.key_var.get().strip(), title="✅ 画图", subtitle="Codex 通知测试", body="连接成功：项目标题、中文正文与高级参数均正常。", group=settings["group"], level=settings["level"], sound=settings["sound"], timeout=float(settings["request_timeout"]), icon=settings["bark_icon"], url=settings["click_url"], encryption_key=encryption_key, encryption_algorithm=settings["encryption_algorithm"])
            except Exception as exc:
                self.after(0, lambda: self._test_result(False, str(exc)))
            else:
                self.after(0, lambda: self._test_result(True, "Bark 测试通知已发送。"))
        threading.Thread(target=worker, daemon=True).start()

    def _test_result(self, success: bool, text: str) -> None:
        self.status_var.set(text)
        self.refresh_all()
        (messagebox.showinfo if success else messagebox.showerror)("测试成功" if success else "测试失败", text)

    def test_hook_chain(self) -> None:
        if not self.save_all(quiet=True):
            return
        event = {"session_id": "environment-test", "turn_id": "environment-test", "cwd": str(PROJECT_ROOT.parent), "hook_event_name": "Stop", "stop_hook_active": False, "last_assistant_message": "完整 Hook 链路模拟成功：UTF-8、项目识别和 Bark 推送均正常。"}
        self.status_var.set("正在模拟完整 Hook 链路…")
        def worker() -> None:
            result = subprocess.run([sys.executable, str(PROJECT_ROOT / "hook_runner.py"), "--codex-bark-notifier"], input=json.dumps(event, ensure_ascii=False).encode("utf-8"), capture_output=True, check=False)
            ok = result.returncode == 0 and result.stdout.strip() == b"{}" and not result.stderr.strip()
            detail = "完整 Hook 模拟通过，并已发送通知。" if ok else result.stderr.decode("utf-8", errors="replace") or "Hook 返回异常。"
            self.after(0, lambda: self._test_result(ok, detail))
        threading.Thread(target=worker, daemon=True).start()

    def refresh_all(self) -> None:
        self._render_projects()
        try:
            status = hook_status()
        except Exception as exc:
            status = {"installed": False, "path_current": False, "interpreter_current": False, "hooks_disabled": False, "python_exists": False, "runner_exists": False, "hooks_path": str(exc)}
        health = load_hook_health()
        verified = self._verified(status, health)
        key_ready = bool(self.key_var.get().strip())
        project = str(health.get("project") or "尚无真实记录")
        self.metric_values["project"].configure(text=project, text_color=TEXT if health else MUTED)
        self.metric_values["hook"].configure(text="真实调用已验证" if verified else "等待真实验证" if status.get("installed") else "尚未安装", text_color=GREEN if verified else AMBER)
        self.metric_values["bark"].configure(text="Key 已配置" if key_ready else "尚未配置", text_color=GREEN if key_ready else RED)
        names = [str(item.get("name") or Path(str(item.get("path", ""))).name) for item in self.projects if item.get("enabled", True)]
        self.project_summary.configure(text=f"{self.scope_var.get()}\n\n" + ("已配置项目：" + "、".join(names[:8]) if names else "工作目录名称会自动作为通知标题，无需逐个配置。"))
        self.hero_subtitle.configure(text=f"通知范围：{self.scope_var.get()}  ·  最近项目：{project}")
        event_counts = counts()
        queued = event_counts.get("queued", 0) + event_counts.get("retrying", 0) + event_counts.get("sending", 0)
        self.project_summary.configure(text=self.project_summary.cget("text") + f"\n\n发送队列：{queued}　已发送：{event_counts.get('sent', 0)}　失败：{event_counts.get('failed', 0)}")
        self._render_history()
        self._update_preview()
        self._render_checks(status, health, verified, key_ready)

    def _validate_advanced(self, settings: dict) -> None:
        for field, value in (("开始时间", settings["quiet_start"]), ("结束时间", settings["quiet_end"])):
            try:
                hour, minute = map(int, str(value).split(":"))
                if not (0 <= hour <= 23 and 0 <= minute <= 59):
                    raise ValueError
            except ValueError as exc:
                raise ValueError(f"{field}必须使用 HH:MM，例如 22:00。") from exc
        if settings["encryption_enabled"]:
            expected = 16 if settings["encryption_algorithm"] == "AES-128-CBC" else 32
            if len(self.encryption_key_var.get().encode("utf-8")) != expected:
                raise ValueError(f"{settings['encryption_algorithm']} 密钥必须正好是 {expected} 个 UTF-8 字节。")

    def _update_preview(self) -> None:
        if not hasattr(self, "preview_title"):
            return
        template = self.title_var.get().strip() or "{project}"
        try:
            title = template.format(project="示例项目", status="任务回合结束", icon="✅")
        except (KeyError, ValueError):
            title = "示例项目"
        mode = MESSAGE_LABELS.get(self.message_mode_var.get(), "summary_200")
        body = self.fixed_message_var.get() if mode == "fixed" else "任务已经完成，所有检查均已通过。"
        if mode == "minimal":
            body = "Codex 状态：任务回合结束。"
        self.preview_title.configure(text=f"✅ {title}")
        self.preview_body.configure(text=body)

    def _verified(self, status: dict, health: dict) -> bool:
        if not all((status.get("installed"), status.get("path_current"), status.get("interpreter_current"), health.get("last_success_at"))):
            return False
        try:
            return datetime.fromisoformat(str(health["last_success_at"])).timestamp() >= Path(str(status["hooks_path"])).stat().st_mtime
        except (OSError, ValueError, KeyError):
            return False

    def _render_checks(self, status: dict, health: dict, verified: bool, key_ready: bool) -> None:
        for child in self.check_list.winfo_children():
            child.destroy()
        checks = [
            ("Bark Key", key_ready, "已加密保存" if key_ready else "请在通知设置中填写"),
            ("Hook 配置", bool(status.get("installed")), str(status.get("hooks_path", ""))),
            ("项目路径", bool(status.get("path_current")), "已指向当前项目" if status.get("path_current") else "请安装或修复 Hook"),
            ("Conda 解释器", bool(status.get("interpreter_current")), str(status.get("python_executable", ""))),
            ("Hooks 功能", not bool(status.get("hooks_disabled")), "config.toml 未关闭 Hooks" if not status.get("hooks_disabled") else "config.toml 中 hooks=false"),
            ("通知程序", bool(status.get("runner_exists")), str(PROJECT_ROOT / "hook_runner.py")),
            ("真实 Hook 调用", verified, f"最近成功：{health.get('last_success_at', '尚无')}" if verified else "请审核 Hook，然后完成一个真实 Codex 任务"),
        ]
        for row, (name, ok, detail) in enumerate(checks):
            item = ctk.CTkFrame(self.check_list, height=64, corner_radius=10, fg_color="#F8FAFC", border_width=1, border_color=BORDER)
            item.grid(row=row, column=0, sticky="ew", padx=6, pady=5)
            item.grid_columnconfigure(1, weight=1)
            ctk.CTkLabel(item, text="✓" if ok else "!", width=34, height=34, corner_radius=17, fg_color=GREEN_BG if ok else AMBER_BG, text_color=GREEN if ok else AMBER, font=(FONT, 14, "bold")).grid(row=0, column=0, rowspan=2, padx=14, pady=14)
            ctk.CTkLabel(item, text=name, text_color=TEXT, font=(FONT, 12, "bold"), anchor="w").grid(row=0, column=1, sticky="sw", pady=(9, 0))
            ctk.CTkLabel(item, text=detail, text_color=MUTED, font=(FONT, 10), anchor="w", wraplength=650).grid(row=1, column=1, sticky="nw", pady=(0, 9))
            ctk.CTkLabel(item, text="正常" if ok else "需处理", width=72, height=28, corner_radius=14, fg_color=GREEN_BG if ok else AMBER_BG, text_color=GREEN if ok else AMBER, font=(FONT, 10, "bold")).grid(row=0, column=2, rowspan=2, padx=14)

    def _render_history(self) -> None:
        if not hasattr(self, "history_list"):
            return
        for child in self.history_list.winfo_children():
            child.destroy()
        rows = history(100)
        state_counts = counts()
        pending = state_counts.get("queued", 0) + state_counts.get("retrying", 0) + state_counts.get("sending", 0)
        self.history_summary.configure(text=f"待发送 {pending}　·　已发送 {state_counts.get('sent', 0)}　·　失败 {state_counts.get('failed', 0)}")
        if not rows:
            ctk.CTkLabel(self.history_list, text="尚无通知历史。完成一次真实任务后会显示在这里。", text_color=MUTED, font=(FONT, 12)).grid(row=0, column=0, pady=50)
            return
        status_style = {
            "sent": ("已发送", GREEN, GREEN_BG), "failed": ("失败", RED, RED_BG),
            "queued": ("排队中", AMBER, AMBER_BG), "retrying": ("等待重试", AMBER, AMBER_BG),
            "sending": ("发送中", PRIMARY, "#DBEAFE"), "suppressed": ("已静默", MUTED, "#E2E8F0"),
        }
        for index, row in enumerate(rows):
            state, color, color_bg = status_style.get(str(row["status"]), (str(row["status"]), MUTED, "#E2E8F0"))
            item = ctk.CTkFrame(self.history_list, corner_radius=11, fg_color="#F8FAFC", border_width=1, border_color=BORDER)
            item.grid(row=index, column=0, sticky="ew", padx=6, pady=5)
            item.grid_columnconfigure(1, weight=1)
            icon = "🔐" if row["event_type"] == "PermissionRequest" else "✓"
            ctk.CTkLabel(item, text=icon, width=42, height=42, corner_radius=12, fg_color="#E8F0FF", text_color=PRIMARY, font=(FONT, 15, "bold")).grid(row=0, column=0, rowspan=2, padx=14, pady=13)
            created = datetime.fromtimestamp(int(row["created_at"])).strftime("%m-%d %H:%M:%S")
            ctk.CTkLabel(item, text=f"{row['project']} · {row['subtitle']}", text_color=TEXT, font=(FONT, 12, "bold"), anchor="w").grid(row=0, column=1, sticky="sw", pady=(10, 0))
            detail = str(row["error"] or row["body"]).replace("\n", " ")[:160]
            ctk.CTkLabel(item, text=f"{created}　{detail}", text_color=MUTED, font=(FONT, 10), anchor="w").grid(row=1, column=1, sticky="nw", pady=(1, 10))
            ctk.CTkLabel(item, text=state, width=76, height=28, corner_radius=14, fg_color=color_bg, text_color=color, font=(FONT, 10, "bold")).grid(row=0, column=2, rowspan=2, padx=14)
            if row["status"] == "failed":
                self._secondary(item, "重试", lambda event_id=int(row["id"]): self.retry_one(event_id), 64).grid(row=0, column=3, rowspan=2, padx=(0, 14))

    def retry_one(self, event_id: int) -> None:
        retry_failed(event_id)
        self._drain_queue()

    def retry_all_failed(self) -> None:
        total = retry_failed()
        self.status_var.set(f"已将 {total} 条失败通知重新加入队列")
        self._drain_queue()

    def clear_finished_history(self) -> None:
        if not messagebox.askyesno("清理历史", "将删除已发送、失败和已静默的历史记录；正在排队的通知会保留。是否继续？"):
            return
        total = clear_history()
        self.status_var.set(f"已清理 {total} 条历史记录")
        self.refresh_all()

    def open_setup_wizard(self) -> None:
        if self._wizard is not None and self._wizard.winfo_exists():
            self._wizard.lift()
            return
        self._wizard = SetupWizard(self)

    def _render_projects(self) -> None:
        for child in self.project_list.winfo_children():
            child.destroy()
        if not self.projects:
            empty = ctk.CTkFrame(self.project_list, height=160, corner_radius=12, fg_color="#F8FAFC", border_width=1, border_color=BORDER)
            empty.grid(row=0, column=0, sticky="ew", padx=8, pady=8)
            ctk.CTkLabel(empty, text="还没有单独配置项目", text_color=TEXT, font=(FONT, 15, "bold")).pack(pady=(34, 4))
            ctk.CTkLabel(empty, text="当前为“所有项目”时，会自动使用工作目录名称作为通知标题。", text_color=MUTED, font=(FONT, 11)).pack()
            return
        for index, project in enumerate(self.projects):
            enabled = bool(project.get("enabled", True))
            path = str(project.get("path", ""))
            name = str(project.get("name") or Path(path).name)
            item = ctk.CTkFrame(self.project_list, height=82, corner_radius=12, fg_color="#F8FAFC", border_width=1, border_color=BORDER)
            item.grid(row=index, column=0, sticky="ew", padx=8, pady=6)
            item.grid_columnconfigure(1, weight=1)
            ctk.CTkLabel(item, text=name[:1].upper() or "P", width=44, height=44, corner_radius=12, fg_color="#E8F0FF", text_color=PRIMARY, font=(FONT, 16, "bold")).grid(row=0, column=0, rowspan=2, padx=16, pady=16)
            ctk.CTkLabel(item, text=name, text_color=TEXT, font=(FONT, 14, "bold"), anchor="w").grid(row=0, column=1, sticky="sw", pady=(13, 0))
            ctk.CTkLabel(item, text=path, text_color=MUTED, font=(FONT, 10), anchor="w").grid(row=1, column=1, sticky="nw", pady=(1, 13))
            ctk.CTkLabel(item, text="启用" if enabled else "禁用", width=62, height=28, corner_radius=14, fg_color=GREEN_BG if enabled else "#E2E8F0", text_color=GREEN if enabled else MUTED, font=(FONT, 10, "bold")).grid(row=0, column=2, rowspan=2, padx=6)
            self._secondary(item, "改名", lambda i=index: self.rename_project(i), 64).grid(row=0, column=3, rowspan=2, padx=4)
            self._secondary(item, "切换", lambda i=index: self.toggle_project(i), 64).grid(row=0, column=4, rowspan=2, padx=4)
            self._secondary(item, "删除", lambda i=index: self.remove_project(i), 64).grid(row=0, column=5, rowspan=2, padx=(4, 14))

    def add_project(self) -> None:
        path = filedialog.askdirectory(title="选择 Codex 项目目录")
        if not path:
            return
        normalized = os.path.normcase(os.path.abspath(path))
        if any(os.path.normcase(os.path.abspath(str(item.get("path", "")))) == normalized for item in self.projects):
            messagebox.showinfo("项目已存在", "该目录已经在列表中。")
            return
        name = simpledialog.askstring("通知标题", "这个项目在 Bark 中显示什么标题？", initialvalue=Path(path).name)
        self.projects.append({"path": path, "name": (name or Path(path).name).strip(), "enabled": True})
        self._render_projects()

    def rename_project(self, index: int) -> None:
        name = simpledialog.askstring("修改标题", "Bark 通知中的项目标题：", initialvalue=str(self.projects[index].get("name", "")))
        if name is not None:
            self.projects[index]["name"] = name.strip() or Path(str(self.projects[index].get("path", ""))).name
            self._render_projects()

    def toggle_project(self, index: int) -> None:
        self.projects[index]["enabled"] = not self.projects[index].get("enabled", True)
        self._render_projects()

    def remove_project(self, index: int) -> None:
        self.projects.pop(index)
        self._render_projects()

    def install_or_repair_hook(self) -> None:
        if not self.key_var.get().strip():
            messagebox.showwarning("缺少 Bark Key", "请先配置 Bark Key。")
            self.show_page("settings")
            return
        if not self.save_all(quiet=True):
            return
        try:
            path, backup = install_hook()
            self.refresh_all()
            messagebox.showinfo("Hook 已安装", f"已写入：{path}\n备份：{backup or '首次创建，无需备份'}\n\n下一步请审核并信任 Hook。")
        except Exception as exc:
            messagebox.showerror("Hook 安装失败", str(exc))

    def remove_hook(self) -> None:
        if not messagebox.askyesno("卸载 Hook", "只删除本程序的 Hook，保留其他 Hook。是否继续？"):
            return
        try:
            path, backup, removed = uninstall_hook()
            self.refresh_all()
            messagebox.showinfo("卸载完成", f"已删除 {removed} 个处理器。\n配置：{path}\n备份：{backup or '无'}")
        except Exception as exc:
            messagebox.showerror("卸载失败", str(exc))

    def open_hook_review(self) -> None:
        launcher = PROJECT_ROOT / "review_hooks.bat"
        try:
            subprocess.Popen(["cmd.exe", "/c", str(launcher)], cwd=str(PROJECT_ROOT), creationflags=getattr(subprocess, "CREATE_NEW_CONSOLE", 0))
            self.status_var.set("审核窗口已打开 · 信任后完成一个真实 Codex 任务")
        except OSError as exc:
            messagebox.showerror("启动失败", str(exc))

    def open_log_dir(self) -> None:
        ensure_runtime_dirs()
        os.startfile(str(LOG_FILE.parent))

    def toggle_key(self) -> None:
        self._key_visible = not self._key_visible
        self.key_entry.configure(show="" if self._key_visible else "●")

    def toggle_encryption_key(self) -> None:
        self._encryption_visible = not self._encryption_visible
        self.encryption_key_entry.configure(show="" if self._encryption_visible else "●")

    def generate_encryption_key(self) -> None:
        length = 16 if self.encryption_algorithm_var.get() == "AES-128-CBC" else 32
        alphabet = string.ascii_letters + string.digits
        self.encryption_key_var.set("".join(secrets.choice(alphabet) for _ in range(length)))
        self.status_var.set(f"已生成 {length} 字节密钥；请在 Bark App 中设置相同密钥")

    def _enabled_changed(self) -> None:
        self.save_all(quiet=True)

    def toggle_enabled_from_tray(self) -> None:
        self.enabled_var.set(not self.enabled_var.get())
        self.save_all(quiet=True)

    def hide_to_tray(self) -> None:
        self.withdraw()
        self.status_var.set("已缩小到系统托盘")

    def show_window(self) -> None:
        self.deiconify()
        self.lift()
        self.focus_force()
        self.refresh_all()

    def quit_app(self) -> None:
        self._really_quit = True
        self.tray.stop()
        self.destroy()

    def _periodic_refresh(self) -> None:
        if not self._really_quit:
            self._drain_queue()
            self.after(15000, self._periodic_refresh)

    def _drain_queue(self) -> None:
        if self._dispatching:
            return
        self._dispatching = True
        def worker() -> None:
            try:
                results = process_due_events(limit=10)
            finally:
                self.after(0, lambda: self._queue_finished(locals().get("results", [])))
        threading.Thread(target=worker, daemon=True).start()

    def _queue_finished(self, results: list[dict]) -> None:
        self._dispatching = False
        if results:
            sent = sum(1 for item in results if item.get("sent"))
            failed = len(results) - sent
            self.status_var.set(f"队列处理完成：发送 {sent}，等待重试/失败 {failed}")
        self.refresh_all()


class SetupWizard(ctk.CTkToplevel):
    STEPS = (
        ("欢迎使用", "这个向导会完成 Bark、Codex Hook 和真实回调验证。所有密钥只在本机加密保存。"),
        ("1 · 配置 Bark", "前往“通知设置”填写设备 Key，然后保存并发送测试通知。"),
        ("2 · 安装 Hook", "安装 Stop 与 PermissionRequest 两个异步 Hook；现有其他 Hook 会保留。"),
        ("3 · 审核信任", "打开 Hook 审核窗口，核对路径后由你亲自信任。程序不会自动绕过信任。"),
        ("4 · 真实验证", "回到 Codex 完成一个简单任务。概览显示“真实调用已验证”后即可完成设置。"),
    )

    def __init__(self, app: NotifierApp) -> None:
        super().__init__(app)
        self.app = app
        self.index = 0
        self.title("Codex Notify 设置向导")
        self.geometry("620x430")
        self.resizable(False, False)
        self.configure(fg_color=BG)
        self.transient(app)
        self.grab_set()
        self.protocol("WM_DELETE_WINDOW", self._later)
        self.content = ctk.CTkFrame(self, fg_color="transparent")
        self.content.pack(fill="both", expand=True, padx=34, pady=28)
        self._render()

    def _render(self) -> None:
        for child in self.content.winfo_children():
            child.destroy()
        title, description = self.STEPS[self.index]
        ctk.CTkLabel(self.content, text=f"设置向导　{self.index + 1}/{len(self.STEPS)}", text_color=PRIMARY, font=(FONT, 11, "bold"), anchor="w").pack(fill="x")
        ctk.CTkLabel(self.content, text=title, text_color=TEXT, font=(FONT, 24, "bold"), anchor="w").pack(fill="x", pady=(18, 8))
        ctk.CTkLabel(self.content, text=description, text_color=MUTED, font=(FONT, 13), justify="left", anchor="nw", wraplength=530).pack(fill="x")
        action_box = ctk.CTkFrame(self.content, corner_radius=12, fg_color=SURFACE, border_width=1, border_color=BORDER)
        action_box.pack(fill="x", pady=24)
        if self.index == 1:
            self.app._primary(action_box, "打开通知设置", lambda: self._open_page("settings"), 140).pack(pady=18)
        elif self.index == 2:
            self.app._primary(action_box, "安装 / 修复 Hook", self.app.install_or_repair_hook, 150).pack(pady=18)
        elif self.index == 3:
            self.app._primary(action_box, "审核 / 信任 Hook", self.app.open_hook_review, 150).pack(pady=18)
        else:
            ctk.CTkLabel(action_box, text="✓ 可随时在“环境诊断”重新执行这些步骤", text_color=GREEN, font=(FONT, 12, "bold")).pack(pady=20)
        nav = ctk.CTkFrame(self.content, fg_color="transparent")
        nav.pack(side="bottom", fill="x")
        self.app._secondary(nav, "稍后设置", self._later, 90).pack(side="left")
        if self.index > 0:
            self.app._secondary(nav, "上一步", self._previous, 80).pack(side="right", padx=8)
        text = "完成" if self.index == len(self.STEPS) - 1 else "下一步"
        self.app._primary(nav, text, self._next, 90).pack(side="right")

    def _open_page(self, page: str) -> None:
        self.grab_release()
        self.destroy()
        self.app.show_page(page)
        self.app.lift()

    def _previous(self) -> None:
        self.index -= 1
        self._render()

    def _next(self) -> None:
        if self.index < len(self.STEPS) - 1:
            self.index += 1
            self._render()
            return
        self.app.settings["setup_completed"] = True
        self.app.save_all(quiet=True)
        self.destroy()

    def _later(self) -> None:
        self.destroy()
