use codex_notify_core::{
    build_notification, dispatch_due, init_logging, should_notify, AppPaths, AppSettings,
    EventStore, HookEvent,
};
use std::io::{Read, Write};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let paths = AppPaths::discover()?;
    init_logging(&paths, "hook")?;
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    let event: HookEvent =
        serde_json::from_slice(input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&input))?;
    if !matches!(event.hook_event_name.as_str(), "Stop" | "PermissionRequest") {
        return Ok(());
    }
    let settings = AppSettings::load(&paths)?;
    if event.hook_event_name == "PermissionRequest" && !settings.permission_notifications {
        return Ok(());
    }
    if !should_notify(&event.cwd, &settings) {
        return Ok(());
    }
    let notification = build_notification(&event, &settings);
    let store = EventStore::new(&paths)?;
    let (_, created) = store.enqueue(&notification)?;
    if created && !notification.suppressed {
        for result in dispatch_due(&store, &settings, 5)? {
            if result.sent {
                tracing::info!(event_id = result.id, "notification sent");
            } else {
                tracing::warn!(event_id=result.id, error=%result.error, "notification queued for retry");
            }
        }
    }
    let health = serde_json::json!({
        "lastSuccessAt": chrono::Local::now().to_rfc3339(), "sessionId": event.session_id,
        "turnId": event.turn_id, "project": notification.project, "cwd": event.cwd
    });
    let _ = std::fs::write(paths.health_file(), serde_json::to_vec_pretty(&health)?);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Codex notification failed: {error}");
    }
    let _ = std::io::stdout().write_all(b"{}\n");
}
