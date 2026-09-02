<div align="center">
  <img src="apps/desktop/src-tauri/icons/app-icon.svg" width="112" alt="CodexNotify Logo">
  <h1>CodexNotify</h1>
  <p>把 Codex 的任务完成与权限请求，通过 Bark 推送到 iPhone 和 Apple Watch。</p>
  <p>
    <a href="README.md">简体中文</a> ·
    <a href="README_EN.md">English</a> ·
    <a href="https://github.com/Xiiiing/CodexNotify/releases/latest">下载最新版</a>
  </p>
</div>

## 项目简介

CodexNotify 是一个面向 Windows、macOS 和 Linux 的轻量通知助手。它监听 Codex 的 `Stop` 与 `PermissionRequest` Hook，在任务结束、等待输入或请求权限时发送 Bark 通知。

- 桌面端使用 Tauri 2 + React + TypeScript，提供设置、历史、诊断和 Hook 管理界面。
- Hook、网络请求、AES 加密、SQLite 队列和敏感信息脱敏均由 Rust 完成。
- Linux 无桌面环境提供独立静态 CLI，不依赖 GTK、WebKitGTK、GIO 或显示服务器。
- 桌面程序关闭后，独立 Hook 仍能继续入队并尝试发送通知。
- Bark Key 与 AES 密钥不会写入 JSON、SQLite 或日志。

## 下载

每个版本只创建一个 GitHub Release 页面，并在同一页面提供四个正式产物：

| 使用环境         | 下载文件                                    | 架构                  | 使用方式                            |
| ---------------- | ------------------------------------------- | --------------------- | ----------------------------------- |
| Windows 10/11    | `CodexNotify-Windows-x64.exe`               | x86_64                | 免安装，双击运行                    |
| macOS            | `CodexNotify-macOS-Universal.dmg`           | Intel + Apple Silicon | 打开 DMG，将应用拖入 Applications   |
| Linux 桌面       | `CodexNotify-Linux-Desktop-x86_64.AppImage` | x86_64                | 添加执行权限后直接运行              |
| Linux 服务器/CLI | `codex-notify-Linux-CLI-x86_64`             | x86_64                | 单文件静态 CLI、Hook 与重试守护程序 |

前往 [Releases](https://github.com/Xiiiing/CodexNotify/releases/latest) 下载最新版。

> Windows 与 macOS 版本目前没有商业代码签名。系统首次启动时可能要求额外确认；请只从本仓库 Release 页面下载。

## 桌面版快速开始

### 1. 启动程序

Windows 直接运行下载的 `.exe`。macOS 打开 `.dmg` 后将 CodexNotify 拖入 Applications。Linux Desktop 执行：

```bash
chmod +x CodexNotify-Linux-Desktop-x86_64.AppImage
./CodexNotify-Linux-Desktop-x86_64.AppImage
```

Linux 桌面版需要可用的 WebKitGTK 桌面环境；没有桌面的服务器请使用 Linux CLI 版本。

### 2. 选择数据保存位置

首次启动会先要求选择非密钥数据的保存位置：

| 模式         | 说明                                                                      |
| ------------ | ------------------------------------------------------------------------- |
| 系统默认     | 推荐。使用当前系统的标准用户级配置、数据和日志目录                        |
| 与程序同目录 | 在可执行文件旁创建 `CodexNotifyData/config`、`data`、`logs`，适合便携使用 |
| 自定义文件夹 | 将全部设置、历史、队列、日志和 Hook 程序保存到指定目录                    |

选择自定义或便携位置时，系统配置目录会保留一个很小的 `storage.json` 位置索引，供独立 Hook 在桌面程序退出后找到数据。该文件不包含 Bark Key、AES 密钥或通知正文。

### 3. 连接 Bark

在首次设置向导中填写 Bark 服务器与 Device Key，发送测试通知。支持官方 Bark、自托管 Bark、HTTP/HTTPS、声音、分组、级别、图标、跳转 URL，以及 AES-128/256-CBC 加密。

桌面版密钥保存在操作系统凭据库中：

- Windows：Credential Manager
- macOS：Keychain
- Linux Desktop：Secret Service，例如 GNOME Keyring 或 KWallet

Linux 没有可用 Secret Service 时，应用会明确提示凭据库不可用，不会降级为明文保存。

### 4. 安装并信任 Hook

1. 打开 CodexNotify 的“系统”页面。
2. 点击“安装 / 修复”。
3. 回到 Codex，输入 `/hooks`。
4. 分别选择 `PermissionRequest` 和 `Stop`，按 `T` 完成信任。
5. 回到 CodexNotify，点击“检查信任”。

CodexNotify 不会代替用户写入信任状态。Hook 配置发生变化后，Codex 可能要求重新审核。

## Linux 无桌面版

CLI 产物是完全静态的 x86_64 Linux 可执行文件，不要求桌面组件。首次安装示例：

```bash
chmod +x codex-notify-Linux-CLI-x86_64
mkdir -p ~/.local/bin
mv codex-notify-Linux-CLI-x86_64 ~/.local/bin/codex-notify

codex-notify init
export CODEX_NOTIFY_BARK_KEY='你的 Bark Device Key'
codex-notify test
codex-notify hook install
codex-notify hook status
```

然后在 Codex 中输入 `/hooks`，信任 `PermissionRequest` 和 `Stop`。Codex 进程必须能够读取 `CODEX_NOTIFY_BARK_KEY` 环境变量；如启用 AES，可同时设置 `CODEX_NOTIFY_ENCRYPTION_KEY`。

常用命令：

```bash
codex-notify status
codex-notify config show
codex-notify config set bark-server https://api.day.app
codex-notify events list 20
codex-notify events retry all
codex-notify events clear
codex-notify daemon
codex-notify daemon --once
codex-notify hook uninstall
```

`daemon` 会持续处理到期重试队列，适合交给 systemd、Supervisor 或其他进程管理器；`daemon --once` 只处理当前到期任务并退出。即使不运行 daemon，后续 Hook 触发时也会顺带发送少量到期事件。

## 数据、密钥与迁移

| 内容                     | 保存位置                           |
| ------------------------ | ---------------------------------- |
| 普通设置                 | `config/settings.json`             |
| 通知历史、去重与重试队列 | `data/events.sqlite3`              |
| 独立 Hook 与健康状态     | `data` 目录                        |
| 运行日志                 | `logs` 目录                        |
| Bark Device Key          | 系统凭据库；CLI 也可从环境变量读取 |
| Bark AES 密钥            | 系统凭据库；CLI 也可从环境变量读取 |

可在“系统 → 数据保存位置 → 更换位置”中迁移数据。迁移流程会：

1. 复制并校验设置、SQLite 历史与队列、日志和 Hook；
2. 验证成功后切换共享位置索引；
3. 自动修复已安装 Hook 的绝对路径；
4. 删除原位置中的 `config`、`data` 和 `logs` 内容；
5. 重启应用。

目标目录必须为空。凭据库中的密钥不属于这些目录，因此不会被迁移或删除。Hook 路径改变后可能需要在 Codex 中重新信任。

高级用户可使用 `CODEX_NOTIFY_DATA_DIR` 强制指定全部非密钥数据的根目录。该环境变量优先级最高，启用期间不能通过桌面界面更换位置。

## 工作原理

```text
Codex
  └─ JSON/stdin → 独立 Rust Hook
                    ├─ 事件分类、过滤与脱敏
                    ├─ SQLite 去重、可靠队列与重试
                    ├─ 系统凭据库 / 环境变量
                    └─ Bark HTTP(S) + 可选 AES

桌面程序
  └─ React 界面 → Tauri Rust 后端
                    ├─ 托盘、单实例与自启动
                    ├─ Hook 安装、修复、卸载与诊断
                    └─ 后台到期队列处理
```

Hook 始终以成功协议响应 Codex；通知失败只会写入日志和可靠队列，不会阻断 Codex。`PermissionRequest` 仅发送通知，不会自动允许或拒绝操作。

## 功能概览

- Bark 服务器、标题、正文模式、声音、分组、级别、图标和跳转链接
- AES-128/256-CBC 加密推送
- 全部项目、包含规则、排除规则和项目别名
- 中文/英文界面、系统主题、浅色和深色主题
- 安静时段、正文裁剪与敏感信息脱敏
- SQLite 唯一去重、离线队列、指数退避、失败重试和历史清理
- Hook 安装、修复、卸载、信任检查与环境诊断
- 托盘驻留、单实例、登录自启动和低频后台工作

## 源码开发

需要当前稳定版 Rust、Node.js 22+、npm，以及对应平台的 [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)。Ubuntu/Debian 桌面构建依赖：

```bash
sudo apt-get install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf pkg-config
```

运行桌面开发版：

```bash
cd apps/desktop
npm ci
npm run tauri:dev
```

运行测试：

```bash
cargo fmt --all -- --check
cargo test -p codex-notify-core -p codex-notify-hook -p codex-notify-cli
cd apps/desktop
npm test
npm run build
```

仓库是一个 Cargo Workspace：

- `crates/codex-notify-core`：共享设置、事件、Bark、加密、SQLite、凭据库和 Hook 配置逻辑
- `apps/hook`：桌面版嵌入的独立 Hook
- `apps/cli`：Linux 无桌面 CLI、Hook 与 daemon
- `apps/desktop/src-tauri`：Tauri 后端
- `apps/desktop/src`：React + TypeScript 前端

## 发布与许可

推送 `v*` 标签后，GitHub Actions 会在一个 Release 中生成并核对四个平台产物。Windows 与 macOS 正式代码签名、安装包和自动更新暂未提供。

本项目采用 [MIT License](LICENSE)。
