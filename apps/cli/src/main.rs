use codex_notify_core::hooks;
use codex_notify_core::security::{self, BARK_KEY_ACCOUNT, ENCRYPTION_KEY_ACCOUNT};
use codex_notify_core::{
    bark, dispatch_due, init_logging, process_hook_input, AppPaths, AppSettings, CoreError,
    CoreResult, EventStore, Notification,
};
use std::io::{Read, Write};
use std::path::PathBuf;

const HELP: &str = r#"CodexNotify Headless

Usage:
  codex-notify init [--server URL]
  codex-notify status
  codex-notify config show
  codex-notify config set <key> <value>
  codex-notify hook <install|status|uninstall>
  codex-notify test
  codex-notify events list [limit]
  codex-notify events retry <id|all>
  codex-notify events clear
  codex-notify daemon [--once]

Required environment variable:
  CODEX_NOTIFY_BARK_KEY

Optional when Bark AES encryption is enabled:
  CODEX_NOTIFY_ENCRYPTION_KEY
"#;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|value| value == hooks::NEW_MARKER) {
        run_hook_protocol();
        return;
    }
    if let Err(error) = run(&args) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run_hook_protocol() {
    let result = (|| -> CoreResult<()> {
        let paths = AppPaths::discover()?;
        init_logging(&paths, "hook")?;
        let mut input = Vec::new();
        std::io::stdin().read_to_end(&mut input)?;
        process_hook_input(&input)
    })();
    if let Err(error) = result {
        eprintln!("Codex notification failed: {error}");
    }
    let _ = std::io::stdout().write_all(b"{}\n");
}

fn run(args: &[String]) -> CoreResult<()> {
    let paths = AppPaths::discover()?;
    init_logging(&paths, "cli")?;
    match args.first().map(String::as_str) {
        None | Some("help" | "--help" | "-h") => print!("{HELP}"),
        Some("--version" | "version") => println!("CodexNotify {}", env!("CARGO_PKG_VERSION")),
        Some("init") => initialize(&paths, &args[1..])?,
        Some("status") => show_status(&paths)?,
        Some("config") => config(&paths, &args[1..])?,
        Some("hook") => hook(&args[1..])?,
        Some("test") => test_notification(&paths)?,
        Some("events") => events(&paths, &args[1..])?,
        Some("daemon") => daemon(&paths, args.iter().any(|value| value == "--once"))?,
        Some(command) => {
            return Err(CoreError::InvalidConfig(format!(
                "unknown command '{command}'\n\n{HELP}"
            )))
        }
    }
    Ok(())
}

fn initialize(paths: &AppPaths, args: &[String]) -> CoreResult<()> {
    let mut settings = AppSettings::load(paths)?;
    if let Some(index) = args.iter().position(|value| value == "--server") {
        settings.bark_server = args
            .get(index + 1)
            .ok_or_else(|| CoreError::InvalidConfig("--server requires a URL".into()))?
            .clone();
    }
    settings.setup_completed = true;
    settings.save(paths)?;
    println!("Configuration: {}", paths.settings_file().display());
    println!("Set CODEX_NOTIFY_BARK_KEY, then run: codex-notify test");
    Ok(())
}

fn show_status(paths: &AppPaths) -> CoreResult<()> {
    let settings = AppSettings::load(paths)?;
    let store = EventStore::new(paths)?;
    let binary = current_binary()?;
    let hook = hooks::status(&binary)?;
    let counts = store.counts()?;
    println!("enabled: {}", settings.enabled);
    println!("bark server: {}", settings.bark_server);
    let key_status = match security::get_secret(BARK_KEY_ACCOUNT) {
        Ok(Some(_)) => "available",
        Ok(None) => "missing",
        Err(_) => "credential store unavailable; set CODEX_NOTIFY_BARK_KEY",
    };
    println!("bark key: {key_status}");
    println!("hook: {} ({})", hook.trust_status, hook.hooks_path);
    println!(
        "queue: {} queued, {} retrying, {} failed, {} sent",
        counts.queued, counts.retrying, counts.failed, counts.sent
    );
    Ok(())
}

fn config(paths: &AppPaths, args: &[String]) -> CoreResult<()> {
    let mut settings = AppSettings::load(paths)?;
    match args.first().map(String::as_str) {
        Some("show") => println!("{}", serde_json::to_string_pretty(&settings)?),
        Some("set") => {
            let key = args
                .get(1)
                .ok_or_else(|| CoreError::InvalidConfig("missing setting key".into()))?;
            let value = args
                .get(2)
                .ok_or_else(|| CoreError::InvalidConfig("missing setting value".into()))?;
            set_setting(&mut settings, key, value)?;
            settings.save(paths)?;
            println!("saved {key}");
        }
        _ => {
            return Err(CoreError::InvalidConfig(
                "usage: codex-notify config <show|set KEY VALUE>".into(),
            ))
        }
    }
    Ok(())
}

fn set_setting(settings: &mut AppSettings, key: &str, value: &str) -> CoreResult<()> {
    let boolean = || {
        value
            .parse::<bool>()
            .map_err(|_| CoreError::InvalidConfig(format!("{key} expects true or false")))
    };
    match key {
        "enabled" => settings.enabled = boolean()?,
        "bark-server" => settings.bark_server = value.into(),
        "group" => settings.group = value.into(),
        "level" => settings.level = value.into(),
        "sound" => settings.sound = value.into(),
        "bark-markdown" => settings.bark_markdown = boolean()?,
        "bark-image" => settings.bark_image = value.into(),
        "bark-call" => settings.bark_call = boolean()?,
        "bark-auto-copy" => settings.bark_auto_copy = boolean()?,
        "bark-copy" => settings.bark_copy = value.into(),
        "bark-action" => settings.bark_action = value.into(),
        "bark-volume" => {
            settings.bark_volume = Some(
                value
                    .parse()
                    .map_err(|_| CoreError::InvalidConfig("bark-volume expects a number".into()))?,
            )
        }
        "bark-badge" => {
            settings.bark_badge = Some(
                value
                    .parse()
                    .map_err(|_| CoreError::InvalidConfig("bark-badge expects a number".into()))?,
            )
        }
        "bark-archive" => settings.bark_archive = Some(boolean()?),
        "bark-ttl" => {
            settings.bark_ttl = Some(
                value
                    .parse()
                    .map_err(|_| CoreError::InvalidConfig("bark-ttl expects a number".into()))?,
            )
        }
        "message-mode" => settings.message_mode = value.into(),
        "device-name" => settings.device_name = value.into(),
        "permission-notifications" => settings.permission_notifications = boolean()?,
        "user-input-notifications" => settings.user_input_notifications = boolean()?,
        "redact-sensitive" => settings.redact_sensitive = boolean()?,
        "quiet-hours-enabled" => settings.quiet_hours_enabled = boolean()?,
        "quiet-start" => settings.quiet_start = value.into(),
        "quiet-end" => settings.quiet_end = value.into(),
        "quiet-action" => settings.quiet_action = value.into(),
        "request-timeout" => {
            settings.request_timeout = value
                .parse()
                .map_err(|_| CoreError::InvalidConfig("request-timeout expects a number".into()))?
        }
        "retry-limit" => {
            settings.retry_limit = value
                .parse()
                .map_err(|_| CoreError::InvalidConfig("retry-limit expects a number".into()))?
        }
        "encryption-enabled" => settings.encryption_enabled = boolean()?,
        "encryption-algorithm" => settings.encryption_algorithm = value.into(),
        _ => {
            return Err(CoreError::InvalidConfig(format!(
                "unsupported setting '{key}'"
            )))
        }
    }
    settings.validate()
}

fn hook(args: &[String]) -> CoreResult<()> {
    let binary = current_binary()?;
    match args.first().map(String::as_str) {
        Some("install") => {
            let (path, backup) = hooks::install(&binary)?;
            println!("installed: {}", path.display());
            if let Some(path) = backup {
                println!("backup: {}", path.display());
            }
            println!("Open Codex /hooks and trust Stop, PermissionRequest, and PreToolUse.");
        }
        Some("status") => {
            let status = hooks::status(&binary)?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Some("uninstall") => {
            let (path, backup, removed) = hooks::uninstall()?;
            println!("removed {removed} handlers from {}", path.display());
            if let Some(path) = backup {
                println!("backup: {}", path.display());
            }
        }
        _ => {
            return Err(CoreError::InvalidConfig(
                "usage: codex-notify hook <install|status|uninstall>".into(),
            ))
        }
    }
    Ok(())
}

fn test_notification(paths: &AppPaths) -> CoreResult<()> {
    let settings = AppSettings::load(paths)?;
    let bark_key = security::get_secret(BARK_KEY_ACCOUNT)
        .ok()
        .flatten()
        .ok_or_else(|| CoreError::Credential("CODEX_NOTIFY_BARK_KEY is not set".into()))?;
    let encryption_key = if settings.encryption_enabled {
        security::get_secret(ENCRYPTION_KEY_ACCOUNT)?
    } else {
        None
    };
    let event_key = format!("cli-test-{}", chrono::Utc::now().timestamp_millis());
    let notification = Notification {
        bark_id: event_key.clone(),
        event_key,
        event_type: "Test".into(),
        session_id: String::new(),
        turn_id: String::new(),
        project: "CodexNotify".into(),
        cwd: String::new(),
        title: format!("{} · CodexNotify", settings.device_name),
        subtitle: "Headless test".into(),
        body: "Your CodexNotify headless connection is working.".into(),
        group: settings.group.clone(),
        level: settings.level.clone(),
        sound: settings.sound.clone(),
        icon: settings.bark_icon.clone(),
        url: settings.click_url.clone(),
        markdown: settings.bark_markdown,
        image: settings.bark_image.clone(),
        volume: settings.bark_volume,
        badge: settings.bark_badge,
        call: settings.bark_call,
        auto_copy: settings.bark_auto_copy,
        copy: settings.bark_copy.clone(),
        archive: settings.bark_archive,
        ttl: settings.bark_ttl,
        action: settings.bark_action.clone(),
        suppressed: false,
        suppress_reason: String::new(),
    };
    bark::send(
        &notification,
        &settings,
        &bark_key,
        encryption_key.as_deref(),
    )?;
    println!("test notification sent");
    Ok(())
}

fn events(paths: &AppPaths, args: &[String]) -> CoreResult<()> {
    let store = EventStore::new(paths)?;
    match args.first().map(String::as_str) {
        Some("list") => {
            let limit = args
                .get(1)
                .and_then(|value| value.parse().ok())
                .unwrap_or(20);
            for event in store.list(limit, None)? {
                println!(
                    "{}\t{:?}\t{}\t{}\t{}",
                    event.id,
                    event.status,
                    format_time(event.created_at),
                    event.project,
                    event.subtitle
                );
            }
        }
        Some("retry") => {
            let target = args
                .get(1)
                .ok_or_else(|| CoreError::InvalidConfig("missing retry target".into()))?;
            let count = if target == "all" {
                store.retry(None)?
            } else {
                let id = target.parse().map_err(|_| {
                    CoreError::InvalidConfig("retry target must be an id or all".into())
                })?;
                store.retry(Some(id))?
            };
            println!("queued {count} event(s)");
        }
        Some("clear") => println!("cleared {} event(s)", store.clear_history()?),
        _ => {
            return Err(CoreError::InvalidConfig(
                "usage: codex-notify events <list [limit]|retry ID|all|clear>".into(),
            ))
        }
    }
    Ok(())
}

fn daemon(paths: &AppPaths, once: bool) -> CoreResult<()> {
    let store = EventStore::new(paths)?;
    println!("CodexNotify daemon started; press Ctrl+C to stop.");
    loop {
        let settings = AppSettings::load(paths)?;
        let results = dispatch_due(&store, &settings, 20)?;
        for result in results {
            if result.sent {
                println!("sent event {}", result.id);
            } else {
                eprintln!("event {} queued: {}", result.id, result.error);
            }
        }
        if once {
            return Ok(());
        }
        let delay = store.next_due_delay()?.unwrap_or(60).clamp(1, 60);
        std::thread::sleep(std::time::Duration::from_secs(delay));
    }
}

fn current_binary() -> CoreResult<PathBuf> {
    std::env::current_exe().map_err(Into::into)
}

fn format_time(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| timestamp.to_string())
}
