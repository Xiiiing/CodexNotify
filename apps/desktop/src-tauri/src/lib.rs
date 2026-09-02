use codex_notify_core::error::ApiError;
use codex_notify_core::hooks::{self, HookStatus};
use codex_notify_core::security::{self, BARK_KEY_ACCOUNT, ENCRYPTION_KEY_ACCOUNT};
use codex_notify_core::{
    bark, dispatch_due, AppPaths, AppSettings, EventCounts, EventRecord, EventStatus, EventStore,
    Notification, StorageInfo, StorageMode,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

#[derive(Clone)]
struct Backend {
    paths: AppPaths,
    store: EventStore,
    hook_binary: PathBuf,
    storage: StorageInfo,
    _runtime_temp: Option<Arc<tempfile::TempDir>>,
    migrating: Arc<AtomicBool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SecretStatus {
    bark_key_configured: bool,
    encryption_key_configured: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStateDto {
    storage: StorageInfo,
    settings: AppSettings,
    counts: EventCounts,
    secrets: SecretStatus,
    hook: HookStatus,
    health: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Diagnostics {
    storage: StorageInfo,
    settings_readable: bool,
    database_ready: bool,
    credential_store_available: bool,
    hook: HookStatus,
    hook_binary: String,
    hook_binary_exists: bool,
    health: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestBarkInput {
    settings: AppSettings,
    bark_key: Option<String>,
    encryption_key: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TestBarkResult {
    ok: bool,
    elapsed_ms: u128,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TestHookResult {
    ok: bool,
    elapsed_ms: u128,
    delivery_status: String,
    error_code: String,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteDeleteResult {
    deleted: usize,
    failed: usize,
}

type CommandResult<T> = Result<T, ApiError>;
fn api<T>(value: codex_notify_core::CoreResult<T>) -> CommandResult<T> {
    value.map_err(Into::into)
}

fn secret_status() -> codex_notify_core::CoreResult<SecretStatus> {
    Ok(SecretStatus {
        bark_key_configured: security::get_secret(BARK_KEY_ACCOUNT)?.is_some(),
        encryption_key_configured: security::get_secret(ENCRYPTION_KEY_ACCOUNT)?.is_some(),
    })
}

fn secret_status_lossy() -> SecretStatus {
    secret_status().unwrap_or(SecretStatus {
        bark_key_configured: false,
        encryption_key_configured: false,
    })
}
fn read_health(paths: &AppPaths) -> serde_json::Value {
    std::fs::read(paths.health_file())
        .ok()
        .and_then(|v| serde_json::from_slice(&v).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn read_diagnostic_health(paths: &AppPaths) -> serde_json::Value {
    std::fs::read(paths.diagnostic_health_file())
        .ok()
        .and_then(|value| serde_json::from_slice(&value).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

fn hook_status_lossy(binary: &std::path::Path) -> HookStatus {
    hooks::status(binary).unwrap_or_else(|error| HookStatus {
        hooks_path: hooks::hooks_path().display().to_string(),
        exists: hooks::hooks_path().exists(),
        installed: false,
        handler_count: 0,
        installed_events: vec![],
        path_current: false,
        configured_command: error.to_string(),
        trusted: false,
        trust_status: "unknown".into(),
        review_required: false,
        enabled: false,
    })
}

#[tauri::command]
fn get_app_state(state: State<'_, Backend>) -> CommandResult<AppStateDto> {
    api((|| {
        let settings = AppSettings::load(&state.paths)?;
        Ok(AppStateDto {
            storage: state.storage.clone(),
            counts: state.store.counts()?,
            secrets: secret_status_lossy(),
            hook: hook_status_lossy(&state.hook_binary),
            health: read_health(&state.paths),
            settings,
        })
    })())
}

#[tauri::command]
fn select_storage(
    mode: StorageMode,
    custom_path: Option<String>,
    app: AppHandle,
) -> CommandResult<()> {
    api(AppPaths::configure_storage(
        mode,
        custom_path.as_deref().map(std::path::Path::new),
    ))?;
    app.restart();
}

#[tauri::command]
fn migrate_storage(
    mode: StorageMode,
    custom_path: Option<String>,
    app: AppHandle,
    state: State<'_, Backend>,
) -> CommandResult<()> {
    if state.migrating.swap(true, Ordering::SeqCst) {
        return Err(ApiError {
            code: "storageMigrationBusy",
            message: "a storage migration is already running".into(),
        });
    }
    let hook_was_installed = hooks::status(&state.hook_binary)
        .map(|status| status.installed)
        .unwrap_or(false);
    match AppPaths::migrate_storage(mode, custom_path.as_deref().map(std::path::Path::new)) {
        Ok(_) => {
            if hook_was_installed {
                if let Ok((new_paths, _)) = AppPaths::resolve_storage() {
                    let new_hook_binary = locate_hook_binary(&new_paths);
                    if let Err(error) = hooks::install(&new_hook_binary) {
                        tracing::warn!(%error, "storage moved but Hook path repair failed");
                    }
                }
            }
            app.restart()
        }
        Err(error) => {
            state.migrating.store(false, Ordering::SeqCst);
            Err(error.into())
        }
    }
}
#[tauri::command]
fn save_settings(settings: AppSettings, state: State<'_, Backend>) -> CommandResult<()> {
    api(settings.save(&state.paths))
}
#[tauri::command]
fn get_secret_status() -> CommandResult<SecretStatus> {
    api(secret_status())
}
#[tauri::command]
fn set_secret(kind: String, value: String) -> CommandResult<()> {
    let account = if kind == "barkKey" {
        BARK_KEY_ACCOUNT
    } else if kind == "encryptionKey" {
        ENCRYPTION_KEY_ACCOUNT
    } else {
        return Err(ApiError {
            code: "invalidSecretKind",
            message: "unknown secret kind".into(),
        });
    };
    api(security::set_secret(account, value.trim()))
}
#[tauri::command]
fn delete_secret(kind: String) -> CommandResult<()> {
    let account = if kind == "barkKey" {
        BARK_KEY_ACCOUNT
    } else if kind == "encryptionKey" {
        ENCRYPTION_KEY_ACCOUNT
    } else {
        return Err(ApiError {
            code: "invalidSecretKind",
            message: "unknown secret kind".into(),
        });
    };
    api(security::delete_secret(account))
}

#[tauri::command]
async fn test_bark_connection(input: TestBarkInput) -> CommandResult<TestBarkResult> {
    tauri::async_runtime::spawn_blocking(
        move || -> codex_notify_core::CoreResult<TestBarkResult> {
            let started = std::time::Instant::now();
            input.settings.validate()?;
            let key = match input.bark_key.filter(|value| !value.trim().is_empty()) {
                Some(value) => Some(value.trim().to_owned()),
                None => security::get_secret(BARK_KEY_ACCOUNT)?,
            }
            .ok_or(codex_notify_core::CoreError::BarkInvalidKey)?;
            let encryption = if input.settings.encryption_enabled {
                match input.encryption_key.filter(|value| !value.is_empty()) {
                    Some(value) => Some(value),
                    None => security::get_secret(ENCRYPTION_KEY_ACCOUNT)?,
                }
            } else {
                None
            };
            let chinese = input.settings.language == "zh";
            let notification = Notification {
                event_key: "manual-test".into(),
                bark_id: format!(
                    "manual-test-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                ),
                event_type: "Test".into(),
                session_id: String::new(),
                turn_id: String::new(),
                project: "CodexNotify".into(),
                cwd: String::new(),
                title: format!("{} · CodexNotify", input.settings.device_name),
                subtitle: String::new(),
                body: if chinese {
                    "CodexNotify 连接测试成功。"
                } else {
                    "Your CodexNotify connection is working."
                }
                .into(),
                group: input.settings.group.clone(),
                level: input.settings.level.clone(),
                sound: input.settings.sound.clone(),
                icon: input.settings.bark_icon.clone(),
                url: input.settings.click_url.clone(),
                markdown: input.settings.bark_markdown,
                image: input.settings.bark_image.clone(),
                volume: input.settings.bark_volume,
                badge: input.settings.bark_badge,
                call: input.settings.bark_call,
                auto_copy: input.settings.bark_auto_copy,
                copy: input.settings.bark_copy.clone(),
                archive: input.settings.bark_archive,
                ttl: input.settings.bark_ttl,
                action: input.settings.bark_action.clone(),
                suppressed: false,
                suppress_reason: String::new(),
            };
            bark::send_test(&notification, &input.settings, &key, encryption.as_deref())?;
            Ok(TestBarkResult {
                ok: true,
                elapsed_ms: started.elapsed().as_millis(),
            })
        },
    )
    .await
    .map_err(|_| ApiError {
        code: "barkUnreachable",
        message: "the Bark test task could not be completed".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn test_hook_delivery(state: State<'_, Backend>) -> CommandResult<TestHookResult> {
    let binary = state.hook_binary.clone();
    let paths = state.paths.clone();
    tauri::async_runtime::spawn_blocking(move || -> CommandResult<TestHookResult> {
        let started = std::time::Instant::now();
        let turn_id = format!(
            "codex-notify-diagnostic-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let input = serde_json::to_vec(&serde_json::json!({
            "hook_event_name": "Stop",
            "session_id": "codex-notify-diagnostic",
            "turn_id": turn_id,
            "cwd": std::env::current_dir().unwrap_or_default(),
            "last_assistant_message": "CodexNotify end-to-end Hook delivery test.",
            "diagnostic": true
        }))
        .map_err(|error| ApiError {
            code: "invalidJson",
            message: error.to_string(),
        })?;
        let mut child = std::process::Command::new(&binary)
            .arg(codex_notify_core::hooks::NEW_MARKER)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|error| ApiError {
                code: "hookExecutionError",
                message: error.to_string(),
            })?;
        use std::io::Write;
        child
            .stdin
            .take()
            .ok_or(ApiError {
                code: "hookExecutionError",
                message: "Hook stdin is unavailable".into(),
            })?
            .write_all(&input)
            .map_err(|error| ApiError {
                code: "hookExecutionError",
                message: error.to_string(),
            })?;
        child.wait().map_err(|error| ApiError {
            code: "hookExecutionError",
            message: error.to_string(),
        })?;
        let health = read_diagnostic_health(&paths);
        let matching =
            health.get("turnId").and_then(serde_json::Value::as_str) == Some(turn_id.as_str());
        let status = health
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("error");
        let delivery_status = health
            .get("deliveryStatus")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        let error_code = health
            .get("errorCode")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(if matching { "" } else { "hookNotInvoked" })
            .to_owned();
        let message = health
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_owned();
        Ok(TestHookResult {
            ok: matching && status == "success" && delivery_status == "sent",
            elapsed_ms: started.elapsed().as_millis(),
            delivery_status,
            error_code,
            message,
        })
    })
    .await
    .map_err(|_| ApiError {
        code: "hookExecutionError",
        message: "the Hook test task could not be completed".into(),
    })?
}

#[tauri::command]
fn list_events(
    limit: Option<u32>,
    status: Option<EventStatus>,
    state: State<'_, Backend>,
) -> CommandResult<Vec<EventRecord>> {
    api(state.store.list(limit.unwrap_or(100), status))
}
#[tauri::command]
fn retry_event(id: i64, state: State<'_, Backend>) -> CommandResult<usize> {
    api(state.store.retry(Some(id)))
}
#[tauri::command]
fn retry_failed(state: State<'_, Backend>) -> CommandResult<usize> {
    api(state.store.retry(None))
}
#[tauri::command]
fn clear_history(state: State<'_, Backend>) -> CommandResult<usize> {
    api(state.store.clear_history())
}

fn delivery_credentials(
    settings: &AppSettings,
) -> codex_notify_core::CoreResult<(String, Option<String>)> {
    let bark_key = security::get_secret(BARK_KEY_ACCOUNT)?
        .ok_or(codex_notify_core::CoreError::BarkInvalidKey)?;
    let encryption_key = if settings.encryption_enabled {
        Some(
            security::get_secret(ENCRYPTION_KEY_ACCOUNT)?.ok_or_else(|| {
                codex_notify_core::CoreError::InvalidConfig(
                    "Bark encryption is enabled but its key is missing".into(),
                )
            })?,
        )
    } else {
        None
    };
    Ok((bark_key, encryption_key))
}

#[tauri::command]
async fn update_remote_notification(
    id: i64,
    body: String,
    state: State<'_, Backend>,
) -> CommandResult<()> {
    let backend = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> codex_notify_core::CoreResult<()> {
        let settings = AppSettings::load(&backend.paths)?;
        let record = backend.store.get(id)?.ok_or_else(|| {
            codex_notify_core::CoreError::InvalidConfig("notification record was not found".into())
        })?;
        if record.status != EventStatus::Sent {
            return Err(codex_notify_core::CoreError::InvalidConfig(
                "only sent notifications can be updated".into(),
            ));
        }
        let mut notification: Notification = serde_json::from_str(&record.payload_json)?;
        notification.bark_id = if record.bark_id.is_empty() {
            record.event_key.clone()
        } else {
            record.bark_id.clone()
        };
        notification.title = format!("{} · {}", settings.device_name.trim(), record.project);
        notification.body = if settings.redact_sensitive {
            codex_notify_core::redact_sensitive_text(body.trim())
        } else {
            body.trim().to_owned()
        };
        if notification.body.is_empty() {
            return Err(codex_notify_core::CoreError::InvalidConfig(
                "notification body cannot be empty".into(),
            ));
        }
        let (key, encryption) = delivery_credentials(&settings)?;
        bark::send(&notification, &settings, &key, encryption.as_deref())?;
        let payload = serde_json::to_string(&notification)?;
        backend
            .store
            .mark_remote_updated(id, &notification.body, &payload)
    })
    .await
    .map_err(|_| ApiError {
        code: "barkUnreachable",
        message: "the notification update task could not be completed".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn delete_remote_notification(id: i64, state: State<'_, Backend>) -> CommandResult<()> {
    let backend = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> codex_notify_core::CoreResult<()> {
        let settings = AppSettings::load(&backend.paths)?;
        let record = backend.store.get(id)?.ok_or_else(|| {
            codex_notify_core::CoreError::InvalidConfig("notification record was not found".into())
        })?;
        let bark_id = if record.bark_id.is_empty() {
            record.event_key
        } else {
            record.bark_id
        };
        let (key, encryption) = delivery_credentials(&settings)?;
        bark::delete(&bark_id, &settings, &key, encryption.as_deref())?;
        backend.store.mark_remote_deleted(id)
    })
    .await
    .map_err(|_| ApiError {
        code: "barkUnreachable",
        message: "the notification delete task could not be completed".into(),
    })?
    .map_err(Into::into)
}

#[tauri::command]
async fn delete_all_remote_notifications(
    state: State<'_, Backend>,
) -> CommandResult<RemoteDeleteResult> {
    let backend = state.inner().clone();
    tauri::async_runtime::spawn_blocking(
        move || -> codex_notify_core::CoreResult<RemoteDeleteResult> {
            let settings = AppSettings::load(&backend.paths)?;
            let (key, encryption) = delivery_credentials(&settings)?;
            let mut result = RemoteDeleteResult {
                deleted: 0,
                failed: 0,
            };
            for (id, bark_id) in backend.store.remote_notification_ids()? {
                match bark::delete(&bark_id, &settings, &key, encryption.as_deref()) {
                    Ok(_) => {
                        backend.store.mark_remote_deleted(id)?;
                        result.deleted += 1;
                    }
                    Err(error) => {
                        tracing::warn!(event_id = id, %error, "remote Bark deletion failed");
                        result.failed += 1;
                    }
                }
            }
            Ok(result)
        },
    )
    .await
    .map_err(|_| ApiError {
        code: "barkUnreachable",
        message: "the batch notification delete task could not be completed".into(),
    })?
    .map_err(Into::into)
}
#[tauri::command]
fn get_hook_status(state: State<'_, Backend>) -> CommandResult<HookStatus> {
    api(hooks::status(&state.hook_binary))
}
#[tauri::command]
fn install_hook(state: State<'_, Backend>) -> CommandResult<HookStatus> {
    api((|| {
        hooks::install(&state.hook_binary)?;
        hooks::status(&state.hook_binary)
    })())
}
#[tauri::command]
fn uninstall_hook(state: State<'_, Backend>) -> CommandResult<HookStatus> {
    api((|| {
        hooks::uninstall()?;
        hooks::status(&state.hook_binary)
    })())
}

fn application_binary_path() -> std::io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(app_image) = std::env::var_os("APPIMAGE") {
        return Ok(PathBuf::from(app_image));
    }

    let executable = std::env::current_exe()?;
    let executable = dunce::simplified(&executable).to_path_buf();
    #[cfg(target_os = "macos")]
    if let Some(bundle) = executable
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
    {
        return Ok(bundle.to_path_buf());
    }
    Ok(executable)
}

fn uninstall_targets(app: &AppHandle, state: &Backend) -> Vec<PathBuf> {
    let mut targets = vec![
        state.paths.config_dir.clone(),
        state.paths.data_dir.clone(),
        state.paths.log_dir.clone(),
        PathBuf::from(&state.storage.locator_file),
    ];
    // Tauri/WebView keeps a small amount of UI state outside the user-selected data root.
    // Include only identifier-scoped application directories returned by Tauri itself.
    targets.extend(
        [
            app.path().app_config_dir(),
            app.path().app_data_dir(),
            app.path().app_local_data_dir(),
            app.path().app_cache_dir(),
            app.path().app_log_dir(),
        ]
        .into_iter()
        .flatten(),
    );
    if state.storage.mode == StorageMode::Portable {
        let root = PathBuf::from(&state.storage.root);
        if root
            .file_name()
            .is_some_and(|name| name == "CodexNotifyData")
        {
            targets.push(root.clone());
            if let Some(parent) = root.parent() {
                targets.push(parent.join(".codex-notify-portable"));
            }
        }
    }
    if let Ok(binary) = application_binary_path() {
        targets.push(binary);
    }
    targets.sort();
    targets.dedup();
    targets
}

fn validate_uninstall_location(state: &Backend) -> CommandResult<()> {
    let root = PathBuf::from(&state.storage.root);
    // Custom/environment roots are user-controlled. Never recursively remove generic
    // config/data/logs directories directly below a filesystem root (for example `/data`
    // or `D:\data`), even after an explicit UI confirmation.
    if matches!(
        state.storage.mode,
        StorageMode::Custom | StorageMode::Environment
    ) && root.parent().is_none()
    {
        return Err(ApiError {
            code: "applicationUninstallUnsafeLocation",
            message: "the selected application data root is a filesystem root".into(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn schedule_uninstall_cleanup(targets: &[PathBuf]) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;

    fn quote(value: &std::path::Path) -> String {
        format!("'{}'", value.to_string_lossy().replace('\'', "''"))
    }

    let paths = targets
        .iter()
        .map(|path| quote(path))
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; \
         while (Get-Process -Id {} -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 250 }}; \
         foreach ($target in @({paths})) {{ Remove-Item -LiteralPath $target -Recurse -Force -ErrorAction SilentlyContinue }}",
        std::process::id()
    );
    Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000 | 0x0000_0200)
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
fn schedule_uninstall_cleanup(targets: &[PathBuf]) -> std::io::Result<()> {
    let pid = std::process::id().to_string();
    let mut command = Command::new("/bin/sh");
    command.args([
        "-c",
        "pid=$1; shift; while kill -0 \"$pid\" 2>/dev/null; do sleep 1; done; for target do rm -rf -- \"$target\"; done",
        "codex-notify-uninstall",
        &pid,
    ]);
    command.args(targets);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

#[tauri::command]
fn uninstall_application(app: AppHandle, state: State<'_, Backend>) -> CommandResult<()> {
    validate_uninstall_location(&state)?;
    // Refuse to remove application data if the Hook config cannot first be updated safely.
    api(hooks::uninstall().map(|_| ()))?;
    api(security::delete_secret(BARK_KEY_ACCOUNT))?;
    api(security::delete_secret(ENCRYPTION_KEY_ACCOUNT))?;
    app.autolaunch().disable().map_err(|error| ApiError {
        code: "autostartError",
        message: error.to_string(),
    })?;
    schedule_uninstall_cleanup(&uninstall_targets(&app, &state)).map_err(|error| ApiError {
        code: "applicationUninstallError",
        message: error.to_string(),
    })?;

    let exit_app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        exit_app.exit(0);
    });
    Ok(())
}
#[tauri::command]
fn run_diagnostics(state: State<'_, Backend>) -> CommandResult<Diagnostics> {
    api((|| {
        let settings_readable = AppSettings::load(&state.paths).is_ok();
        let database_ready = EventStore::new(&state.paths).is_ok();
        let credential_store_available = secret_status().is_ok();
        let hook = hooks::status(&state.hook_binary)?;
        Ok(Diagnostics {
            storage: state.storage.clone(),
            settings_readable,
            database_ready,
            credential_store_available,
            hook,
            hook_binary: state.hook_binary.display().to_string(),
            hook_binary_exists: state.hook_binary.exists(),
            health: read_health(&state.paths),
        })
    })())
}
#[tauri::command]
fn get_autostart(app: AppHandle) -> CommandResult<bool> {
    app.autolaunch().is_enabled().map_err(|e| ApiError {
        code: "autostartError",
        message: e.to_string(),
    })
}
#[tauri::command]
fn set_autostart(enabled: bool, app: AppHandle) -> CommandResult<()> {
    let launcher = app.autolaunch();
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|e| ApiError {
        code: "autostartError",
        message: e.to_string(),
    })
}

static EMBEDDED_HOOK: &[u8] = include_bytes!(env!("CODEX_NOTIFY_EMBEDDED_HOOK"));

fn embedded_hook_path(paths: &AppPaths) -> std::io::Result<PathBuf> {
    let directory = paths.data_dir.join("bin");
    std::fs::create_dir_all(&directory)?;
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let target = directory.join(format!("codex-notify-hook{extension}"));
    let current = std::fs::read(&target).ok();
    if current.as_deref() != Some(EMBEDDED_HOOK) {
        let temporary = directory.join(format!("codex-notify-hook.new{extension}"));
        std::fs::write(&temporary, EMBEDDED_HOOK)?;
        if target.exists() {
            std::fs::remove_file(&target)?;
        }
        std::fs::rename(temporary, &target)?;
    }
    Ok(target)
}

fn locate_hook_binary(paths: &AppPaths) -> PathBuf {
    if let Some(path) = std::env::var_os("CODEX_NOTIFY_HOOK_PATH") {
        return PathBuf::from(path);
    }
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let current = std::env::current_exe().unwrap_or_default();
    let directory = current
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let sibling = directory.join(format!("codex-notify-hook{extension}"));
    if sibling.exists() {
        return sibling;
    }
    embedded_hook_path(paths)
        .unwrap_or_else(|_| directory.join(format!("binaries/codex-notify-hook{extension}")))
}

fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open CodexNotify", true, None::<&str>)?;
    let enabled = CheckMenuItem::with_id(
        app,
        "enabled",
        "Notifications enabled",
        true,
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &enabled, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().expect("application icon").clone())
        .menu(&menu)
        .tooltip("CodexNotify")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "enabled" => {
                if let Some(state) = app.try_state::<Backend>() {
                    if let Ok(mut settings) = AppSettings::load(&state.paths) {
                        settings.enabled = !settings.enabled;
                        let _ = settings.save(&state.paths);
                    }
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    let (selected_paths, storage) = AppPaths::resolve_storage().expect("application paths");
    // The chooser must be the first persistent write for a new installation. Until the user
    // selects a location, run the backend from a process-scoped temporary directory.
    let runtime_temp = (!storage.configured)
        .then(|| tempfile::tempdir().expect("temporary setup directory"))
        .map(Arc::new);
    let paths = runtime_temp
        .as_ref()
        .map(|temporary| AppPaths::from_root(temporary.path()))
        .unwrap_or(selected_paths);
    let _ = codex_notify_core::init_logging(&paths, "desktop");
    let store = EventStore::new(&paths).expect("event database");
    let hook_binary = if storage.configured {
        locate_hook_binary(&paths)
    } else {
        paths.data_dir.join(if cfg!(windows) {
            "bin/codex-notify-hook.exe"
        } else {
            "bin/codex-notify-hook"
        })
    };
    let backend = Backend {
        paths: paths.clone(),
        store: store.clone(),
        hook_binary,
        storage,
        _runtime_temp: runtime_temp,
        migrating: Arc::new(AtomicBool::new(false)),
    };
    let background = backend.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(backend)
        .setup(move |app| {
            create_tray(app.handle())?;
            let background = background.clone();
            std::thread::spawn(move || loop {
                let delay = background
                    .store
                    .next_due_delay()
                    .ok()
                    .flatten()
                    .map(|seconds| seconds.min(60))
                    .unwrap_or(60);
                if delay > 0 {
                    std::thread::sleep(std::time::Duration::from_secs(delay));
                }
                if background.migrating.load(Ordering::SeqCst) {
                    continue;
                }
                if !matches!(background.store.next_due_delay(), Ok(Some(0))) {
                    continue;
                }
                if let Ok(settings) = AppSettings::load(&background.paths) {
                    match dispatch_due(&background.store, &settings, 10) {
                        Ok(results) if results.is_empty() => {
                            // Missing credentials or another process may own the claim. Avoid a
                            // hot loop while leaving the queued event available for the Hook.
                            std::thread::sleep(std::time::Duration::from_secs(60));
                        }
                        Ok(_) => {}
                        Err(error) => {
                            tracing::warn!(%error,"background dispatch failed");
                            std::thread::sleep(std::time::Duration::from_secs(30));
                        }
                    }
                } else {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            select_storage,
            migrate_storage,
            save_settings,
            get_secret_status,
            set_secret,
            delete_secret,
            test_bark_connection,
            test_hook_delivery,
            list_events,
            retry_event,
            retry_failed,
            clear_history,
            update_remote_notification,
            delete_remote_notification,
            delete_all_remote_notifications,
            get_hook_status,
            install_hook,
            uninstall_hook,
            uninstall_application,
            run_diagnostics,
            get_autostart,
            set_autostart
        ])
        .run(tauri::generate_context!())
        .expect("error while running CodexNotify");
}
