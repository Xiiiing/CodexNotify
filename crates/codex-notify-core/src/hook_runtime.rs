use crate::{
    build_notification, dispatch_due, should_notify, AppPaths, AppSettings, CoreError, CoreResult,
    EventStore, HookEvent,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookHealth<'a> {
    status: &'a str,
    stage: &'a str,
    event_type: &'a str,
    delivery_status: &'a str,
    last_attempt_at: String,
    last_success_at: Option<String>,
    error_code: &'a str,
    message: &'a str,
    session_id: &'a str,
    turn_id: &'a str,
    project: &'a str,
    cwd: &'a str,
}

struct HookOutcome {
    event: HookEvent,
    project: String,
    stage: &'static str,
    delivery_status: &'static str,
}

fn error_details(error: &CoreError) -> (&'static str, &'static str) {
    match error {
        CoreError::Io(_) => (
            "ioError",
            "The Hook could not access its application files.",
        ),
        CoreError::Json(_) => ("invalidHookInput", "Codex sent invalid Hook input."),
        CoreError::Database(_) => ("databaseError", "The Hook queue is unavailable."),
        CoreError::Credential(_) => (
            "credentialError",
            "The Hook could not read the system credential store.",
        ),
        CoreError::InvalidConfig(_) => ("invalidConfig", "The saved settings are invalid."),
        CoreError::TemporaryPortableLocation => (
            "temporaryPortableLocation",
            "Portable storage cannot use a temporary download directory.",
        ),
        CoreError::Network(_) | CoreError::BarkUnreachable => (
            "barkUnreachable",
            "The Hook could not reach the Bark server.",
        ),
        CoreError::BarkInvalidKey => ("barkInvalidKey", "The Bark device key is invalid."),
        CoreError::BarkRejected => ("barkRejected", "Bark rejected the notification."),
        CoreError::BarkTimeout => ("barkTimeout", "The Bark request timed out."),
        CoreError::BarkServer => ("barkServerError", "The Bark server returned an error."),
        CoreError::InvalidBarkServer => {
            ("invalidBarkServer", "The Bark server address is invalid.")
        }
        CoreError::InvalidEncryptionKey => (
            "invalidEncryptionKey",
            "The Bark encryption key is invalid.",
        ),
        CoreError::HookConfig(_) => ("hookConfigError", "The Hook configuration is invalid."),
    }
}

fn write_health(paths: &AppPaths, health: &HookHealth<'_>, diagnostic: bool) -> CoreResult<()> {
    paths.ensure()?;
    let destination = if diagnostic {
        paths.diagnostic_health_file()
    } else {
        paths.health_file()
    };
    let mut temporary = tempfile::NamedTempFile::new_in(&paths.data_dir)?;
    use std::io::Write;
    temporary.write_all(&serde_json::to_vec_pretty(health)?)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(destination)
        .map_err(|error| error.error)?;
    Ok(())
}

pub fn process_hook_input(input: &[u8]) -> CoreResult<()> {
    let paths = AppPaths::discover()?;
    let attempted_at = chrono::Local::now().to_rfc3339();
    let diagnostic = serde_json::from_slice::<serde_json::Value>(input)
        .ok()
        .and_then(|value| value.get("diagnostic").and_then(|value| value.as_bool()))
        .unwrap_or(false);
    let result = process_hook_input_inner(input, &paths);
    match &result {
        Ok(outcome) => {
            write_health(
                &paths,
                &HookHealth {
                    status: "success",
                    stage: outcome.stage,
                    event_type: &outcome.event.hook_event_name,
                    delivery_status: outcome.delivery_status,
                    last_attempt_at: attempted_at.clone(),
                    last_success_at: Some(chrono::Local::now().to_rfc3339()),
                    error_code: "",
                    message: "",
                    session_id: &outcome.event.session_id,
                    turn_id: &outcome.event.turn_id,
                    project: &outcome.project,
                    cwd: &outcome.event.cwd,
                },
                diagnostic,
            )?;
        }
        Err(error) => {
            let (code, message) = error_details(error);
            let _ = write_health(
                &paths,
                &HookHealth {
                    status: "error",
                    stage: "processing",
                    event_type: "",
                    delivery_status: "failed",
                    last_attempt_at: attempted_at,
                    last_success_at: None,
                    error_code: code,
                    message,
                    session_id: "",
                    turn_id: "",
                    project: "",
                    cwd: "",
                },
                diagnostic,
            );
        }
    }
    result.map(|_| ())
}

fn process_hook_input_inner(input: &[u8], paths: &AppPaths) -> CoreResult<HookOutcome> {
    let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
    let event: HookEvent = serde_json::from_slice(input)?;
    let user_input_request = event.hook_event_name == "PreToolUse"
        && matches!(
            event.tool_name.as_str(),
            "request_user_input" | "requestUserInput"
        );
    if !matches!(event.hook_event_name.as_str(), "Stop" | "PermissionRequest")
        && !user_input_request
    {
        return Ok(HookOutcome {
            event,
            project: String::new(),
            stage: "ignored",
            delivery_status: "ignored",
        });
    }

    let settings = AppSettings::load(paths)?;
    if event.hook_event_name == "PermissionRequest" && !settings.permission_notifications {
        return Ok(HookOutcome {
            event,
            project: String::new(),
            stage: "filtered",
            delivery_status: "disabled",
        });
    }
    if user_input_request && !settings.user_input_notifications {
        return Ok(HookOutcome {
            event,
            project: String::new(),
            stage: "filtered",
            delivery_status: "disabled",
        });
    }
    if !event.diagnostic && !should_notify(&event.cwd, &settings) {
        return Ok(HookOutcome {
            event,
            project: String::new(),
            stage: "filtered",
            delivery_status: "filtered",
        });
    }

    let notification = build_notification(&event, &settings);
    let store = EventStore::new(paths)?;
    let (event_id, created) = store.enqueue(&notification)?;
    if created && !notification.suppressed {
        for result in dispatch_due(&store, &settings, 5)? {
            if result.sent {
                tracing::info!(event_id = result.id, "notification sent");
            } else {
                tracing::warn!(event_id=result.id, error=%result.error, "notification queued for retry");
            }
        }
    }
    let delivery_status = store
        .get(event_id)?
        .map(|record| record.status.label())
        .unwrap_or("unknown");
    Ok(HookOutcome {
        event,
        project: notification.project,
        stage: if notification.suppressed {
            "suppressed"
        } else {
            "processed"
        },
        delivery_status,
    })
}
