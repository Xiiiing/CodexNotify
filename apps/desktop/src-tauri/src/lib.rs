use codex_notify_core::error::ApiError;
use codex_notify_core::hooks::{self, HookStatus};
use codex_notify_core::security::{self, BARK_KEY_ACCOUNT, ENCRYPTION_KEY_ACCOUNT};
use codex_notify_core::{
    bark, dispatch_due, AppPaths, AppSettings, EventCounts, EventRecord, EventStatus, EventStore,
    Notification,
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

#[derive(Clone)]
struct Backend {
    paths: AppPaths,
    store: EventStore,
    hook_binary: PathBuf,
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
    settings: AppSettings,
    counts: EventCounts,
    secrets: SecretStatus,
    hook: HookStatus,
    health: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Diagnostics {
    settings_readable: bool,
    database_ready: bool,
    credential_store_available: bool,
    hook: HookStatus,
    hook_binary: String,
    hook_binary_exists: bool,
    health: serde_json::Value,
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
            counts: state.store.counts()?,
            secrets: secret_status_lossy(),
            hook: hook_status_lossy(&state.hook_binary),
            health: read_health(&state.paths),
            settings,
        })
    })())
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
fn test_notification(state: State<'_, Backend>) -> CommandResult<serde_json::Value> {
    api((|| {
        let settings = AppSettings::load(&state.paths)?;
        let key = security::get_secret(BARK_KEY_ACCOUNT)?.ok_or_else(|| {
            codex_notify_core::CoreError::InvalidConfig("Bark device key is missing".into())
        })?;
        let encryption = if settings.encryption_enabled {
            security::get_secret(ENCRYPTION_KEY_ACCOUNT)?
        } else {
            None
        };
        let notification = Notification {
            event_key: "manual-test".into(),
            event_type: "Test".into(),
            session_id: String::new(),
            turn_id: String::new(),
            project: "CodexNotify".into(),
            cwd: String::new(),
            title: "✅ CodexNotify".into(),
            subtitle: "Test notification".into(),
            body: "Your CodexNotify connection is working.".into(),
            group: settings.group.clone(),
            level: settings.level.clone(),
            sound: settings.sound.clone(),
            icon: settings.bark_icon.clone(),
            url: settings.click_url.clone(),
            suppressed: false,
            suppress_reason: String::new(),
        };
        bark::send(&notification, &settings, &key, encryption.as_deref())
    })())
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
#[tauri::command]
fn run_diagnostics(state: State<'_, Backend>) -> CommandResult<Diagnostics> {
    api((|| {
        let settings_readable = AppSettings::load(&state.paths).is_ok();
        let database_ready = EventStore::new(&state.paths).is_ok();
        let credential_store_available = secret_status().is_ok();
        let hook = hooks::status(&state.hook_binary)?;
        Ok(Diagnostics {
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
    let paths = AppPaths::discover().expect("application paths");
    let _ = codex_notify_core::init_logging(&paths, "desktop");
    let store = EventStore::new(&paths).expect("event database");
    let backend = Backend {
        paths: paths.clone(),
        store: store.clone(),
        hook_binary: locate_hook_binary(&paths),
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
            save_settings,
            get_secret_status,
            set_secret,
            delete_secret,
            test_notification,
            list_events,
            retry_event,
            retry_failed,
            clear_history,
            get_hook_status,
            install_hook,
            uninstall_hook,
            run_diagnostics,
            get_autostart,
            set_autostart
        ])
        .run(tauri::generate_context!())
        .expect("error while running CodexNotify");
}
