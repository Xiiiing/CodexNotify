# CodexNotify

CodexNotify sends Codex lifecycle events to Bark. It ships as a Tauri desktop companion for graphical systems and as a single-file Rust CLI for headless Linux. All privileged work lives in Rust, including the Codex Hook, HTTP, encryption, credential access and the persistent SQLite queue.

CodexNotify 是一个跨平台桌面通知工具，通过 Bark 将 Codex 的任务完成和审批请求发送到 iPhone 与 Apple Watch。界面使用 Tauri + React + TypeScript，Hook、网络、加密、系统凭据库和 SQLite 可靠队列均由 Rust 实现。

## Architecture

```text
Codex → JSON/stdin → codex-notify-hook
                         ├─ settings + OS credential store
                         ├─ SQLite queue / deduplication
                         └─ Bark HTTPS

Tauri desktop → Rust commands → shared core
      ├─ React console
      ├─ tray / autostart / single instance
      └─ background retry worker

Headless CLI → shared core
      ├─ configuration / diagnostics / history
      ├─ foreground daemon retry worker
      └─ same executable handles --codex-notify-hook
```

The workspace contains:

- `crates/codex-notify-core`: shared Rust domain logic and persistence.
- `apps/hook`: standalone Codex command Hook.
- `apps/cli`: single-file Linux Headless CLI and Hook.
- `apps/desktop/src-tauri`: privileged desktop backend.
- `apps/desktop/src`: bilingual React console.

Secrets are stored under service `com.xiiiing.codex-notify` in Windows Credential Manager, macOS Keychain, or Linux Secret Service. Headless Linux can inject `CODEX_NOTIFY_BARK_KEY` and `CODEX_NOTIFY_ENCRYPTION_KEY`; environment values take priority and are never copied into settings, SQLite or logs. Linux desktop sessions should provide a Secret Service implementation such as GNOME Keyring or KWallet.

## Release downloads

Each version is published on one GitHub Release page with four ready-to-run products:

- `CodexNotify-macOS-Universal.dmg`: macOS desktop for Intel and Apple Silicon.
- `CodexNotify-Windows-x64.exe`: portable Windows 10/11 desktop executable.
- `CodexNotify-Linux-Desktop-x86_64.AppImage`: graphical Linux desktop edition.
- `codex-notify-Linux-CLI-x86_64`: static headless Linux CLI, Hook and daemon.

The macOS and Windows products require commercial signing credentials to avoid all operating-system security prompts. Until those repository secrets are configured, releases use macOS ad-hoc signing and an unsigned Windows executable. Linux downloads may require `chmod +x <filename>` once after downloading.

## Linux editions

Only two Linux downloads are published:

- `CodexNotify-Linux-Desktop-x86_64.AppImage`: graphical Tauri desktop edition with the Hook embedded.
- `codex-notify-Linux-CLI-x86_64`: statically linked Headless single executable with no GTK, WebKitGTK, GIO or display-server dependency. The same file acts as both management CLI and Codex Hook.

Headless quick start:

```bash
chmod +x codex-notify-Linux-CLI-x86_64
mv codex-notify-Linux-CLI-x86_64 ~/.local/bin/codex-notify
codex-notify init
read -rsp "Bark device key: " CODEX_NOTIFY_BARK_KEY && export CODEX_NOTIFY_BARK_KEY
codex-notify test
codex-notify hook install
codex-notify hook status
```

After installing, open `/hooks` in Codex and trust the `Stop` and `PermissionRequest` handlers. Keep the environment variable available to the Codex process. Run `codex-notify daemon` in a supervisor or terminal when retries must continue without new Hook events; `daemon --once` processes only the currently due queue.

## Development

Requirements:

- Current stable Rust and Cargo
- Node.js 20 or newer and npm
- Platform prerequisites from the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)
- On Ubuntu/Debian: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf pkg-config`

Install and run:

```bash
cd apps/desktop
npm install
npm run tauri:dev
```

`tauri:dev` first builds the standalone Hook and stages the target-triple sidecar. When running binaries manually, build both processes and point the desktop app to the Hook if they are not siblings:

```bash
cargo build -p codex-notify-hook -p codex-notify-desktop
CODEX_NOTIFY_HOOK_PATH="$PWD/target/debug/codex-notify-hook" cargo run -p codex-notify-desktop
```

## Tests

```bash
cargo test -p codex-notify-core -p codex-notify-hook -p codex-notify-cli
cd apps/desktop
npm test
npm run build
```

The repository does not ship signed installers or an update feed yet. CI checks Windows, macOS and Linux source builds.

## Interface and resource profile

The desktop console uses four focused areas: Overview, Push, Rules and System. The repository-native `N` mark is stored as SVG and rendered to PNG, ICO and ICNS assets for the three desktop platforms without a runtime image dependency.

Production builds use size optimization, full LTO, a single codegen unit, symbol stripping and abort-on-panic. The webview stops periodic refreshes while hidden and refreshes visible state at most once per minute. The Rust retry worker queries SQLite for the next due event and stays asleep while the queue is empty; it does not continuously poll Bark or the system credential store.

```bash
cargo build --release -p codex-notify-desktop -p codex-notify-hook
```

## Local data and Hook safety

On a fresh desktop installation, CodexNotify asks where to store all non-secret files before creating its settings or database. The choices are the operating system's standard per-user application directories (recommended), a portable `CodexNotifyData/config|data|logs` directory beside the executable, or a custom folder. Existing installations that already have `settings.json` continue using the standard location without a migration prompt.

For portable and custom locations, a small `storage.json` locator remains in the standard configuration directory so the standalone Hook can resolve the same data directory while the desktop app is closed. It stores only the selected mode and absolute path. Bark and AES keys remain exclusively in Windows Credential Manager, macOS Keychain or Linux Secret Service and are never written into the selected folder or locator.

The System page can move an existing installation to another location. Migration requires an empty destination, uses SQLite's online backup API, verifies the copied settings and database, switches the Hook locator only after validation, repairs an installed Hook to its new absolute binary path, removes the previous `config`, `data` and `logs` contents, and restarts the app. A changed Hook command may require Codex trust review. When moving away from the system-default location, its config directory retains only the small locator required by the standalone Hook. Overlapping paths and environment-controlled storage are rejected. Credential-store entries are not moved or deleted.

For isolated development, headless deployments or tests, set `CODEX_NOTIFY_DATA_DIR` to redirect all non-secret files; this environment override takes precedence over the desktop selection. Set `CODEX_HOME` to test Hook configuration without modifying the real `~/.codex/hooks.json`.

Hook installation is always user initiated. It backs up `hooks.json`, preserves third-party handlers, removes both the legacy `--codex-bark-notifier` marker and the current `--codex-notify-hook` marker, then installs `Stop` and `PermissionRequest` as asynchronous commands. Permission requests are notification-only and never approve or deny an operation.

Codex independently reviews user Hooks before running them. After installation, enter `/hooks` in Codex, select the `PermissionRequest` and `Stop` handlers, and press `T` to trust them. The System page compares Codex's stored trust hashes with the current normalized Hook definitions; use **Check trust** after completing the review. CodexNotify never writes trust approval on the user's behalf.
