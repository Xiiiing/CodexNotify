use crate::{
    build_notification, dispatch_due, should_notify, AppPaths, AppSettings, CoreResult, EventStore,
    HookEvent,
};

pub fn process_hook_input(input: &[u8]) -> CoreResult<()> {
    let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
    let event: HookEvent = serde_json::from_slice(input)?;
    if !matches!(event.hook_event_name.as_str(), "Stop" | "PermissionRequest") {
        return Ok(());
    }

    let paths = AppPaths::discover()?;
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
        "lastSuccessAt": chrono::Local::now().to_rfc3339(),
        "sessionId": event.session_id,
        "turnId": event.turn_id,
        "project": notification.project,
        "cwd": event.cwd
    });
    std::fs::write(paths.health_file(), serde_json::to_vec_pretty(&health)?)?;
    Ok(())
}
