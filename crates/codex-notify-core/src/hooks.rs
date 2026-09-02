use crate::error::{CoreError, CoreResult};
use chrono::Local;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const NEW_MARKER: &str = "--codex-notify-hook";
pub const LEGACY_MARKER: &str = "--codex-bark-notifier";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStatus {
    pub hooks_path: String,
    pub exists: bool,
    pub installed: bool,
    pub handler_count: u32,
    pub installed_events: Vec<String>,
    pub path_current: bool,
    pub configured_command: String,
    pub trusted: bool,
    pub trust_status: String,
    pub review_required: bool,
    pub enabled: bool,
}

fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            directories::BaseDirs::new()
                .map(|d| d.home_dir().join(".codex"))
                .unwrap_or_else(|| PathBuf::from(".codex"))
        })
}
pub fn hooks_path() -> PathBuf {
    codex_home().join("hooks.json")
}

fn config_path() -> PathBuf {
    codex_home().join("config.toml")
}

#[derive(Default)]
struct StoredHookState {
    enabled: bool,
    trusted_hash: Option<String>,
}

fn stored_hook_states() -> Option<BTreeMap<String, StoredHookState>> {
    let path = config_path();
    if !path.exists() {
        return Some(BTreeMap::new());
    }
    let text = std::fs::read_to_string(path).ok()?;
    let document: toml::Value = toml::from_str(&text).ok()?;
    let states = document
        .get("hooks")
        .and_then(|value| value.get("state"))
        .and_then(toml::Value::as_table);
    let mut result = BTreeMap::new();
    for (key, value) in states.into_iter().flatten() {
        result.insert(
            key.clone(),
            StoredHookState {
                enabled: value
                    .get("enabled")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(true),
                trusted_hash: value
                    .get("trusted_hash")
                    .and_then(toml::Value::as_str)
                    .map(ToOwned::to_owned),
            },
        );
    }
    Some(result)
}

fn event_key(event: &str) -> Option<&'static str> {
    match event {
        "PreToolUse" => Some("pre_tool_use"),
        "PermissionRequest" => Some("permission_request"),
        "Stop" => Some("stop"),
        _ => None,
    }
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted = map
                .iter()
                .map(|(key, value)| (key.clone(), canonical_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

/// Match Codex's normalized hook identity hash. Platform-specific command overrides are
/// resolved before hashing, and fields unsupported by these events are omitted.
fn handler_hash(event: &str, source: &Value) -> Option<String> {
    let event_name = event_key(event)?;
    let mut normalized = source.as_object()?.clone();
    let command = if cfg!(windows) {
        normalized
            .get("commandWindows")
            .and_then(Value::as_str)
            .or_else(|| normalized.get("command").and_then(Value::as_str))?
    } else {
        normalized.get("command").and_then(Value::as_str)?
    }
    .to_owned();
    normalized.insert("command".into(), Value::String(command));
    normalized.remove("commandWindows");
    normalized.remove("command_windows");
    normalized.remove("additionalContextLimit");
    let timeout = normalized
        .get("timeout")
        .and_then(Value::as_u64)
        .unwrap_or(600)
        .max(1);
    normalized.insert("timeout".into(), Value::Number(timeout.into()));
    normalized.entry("async").or_insert(Value::Bool(false));
    let identity = json!({
        "event_name": event_name,
        "hooks": [Value::Object(normalized)],
    });
    let bytes = serde_json::to_vec(&canonical_json(&identity)).ok()?;
    Some(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn load(path: &Path) -> CoreResult<Value> {
    if !path.exists() {
        return Ok(json!({"description":"User lifecycle hooks for Codex.","hooks":{}}));
    }
    let mut value: Value = serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|e| CoreError::HookConfig(format!("refusing to overwrite invalid JSON: {e}")))?;
    if !value.is_object() {
        return Err(CoreError::HookConfig(
            "hooks.json must be a JSON object".into(),
        ));
    }
    if value.get("hooks").is_none() {
        value
            .as_object_mut()
            .unwrap()
            .insert("hooks".into(), json!({}));
    }
    if !value.get("hooks").is_some_and(Value::is_object) {
        return Err(CoreError::HookConfig(
            "hooks.json must contain an object named hooks".into(),
        ));
    }
    Ok(value)
}
fn ours(handler: &Value) -> bool {
    let command = format!(
        "{} {}",
        handler.get("command").and_then(Value::as_str).unwrap_or(""),
        handler
            .get("commandWindows")
            .and_then(Value::as_str)
            .unwrap_or("")
    );
    command.contains(NEW_MARKER) || command.contains(LEGACY_MARKER)
}
fn remove_ours(document: &mut Value) -> u32 {
    let mut removed = 0;
    let Some(hooks) = document.get_mut("hooks").and_then(Value::as_object_mut) else {
        return 0;
    };
    for groups in hooks.values_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        groups.retain_mut(|group| {
            let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let old = handlers.len();
            handlers.retain(|h| !ours(h));
            removed += (old - handlers.len()) as u32;
            !handlers.is_empty()
        });
    }
    removed
}
fn quote_posix(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn windows_command(value: &str) -> String {
    // Codex executes hooks through the active session shell. Invoke PowerShell explicitly so
    // this remains valid whether that outer shell is PowerShell or cmd.exe. A quoted path alone
    // is only a string expression in PowerShell; `&` performs the actual invocation.
    format!(
        "powershell.exe -NoLogo -NoProfile -NonInteractive -Command \"& '{}' {}\"",
        value.replace('\'', "''"),
        NEW_MARKER
    )
}
fn handler(binary: &Path) -> Value {
    let path = binary.to_string_lossy();
    json!({"type":"command","command":format!("{} {}",quote_posix(&path),NEW_MARKER),"commandWindows":windows_command(&path),"timeout":30,"async":true,"statusMessage":"Recording Codex notification"})
}
fn backup(path: &Path) -> CoreResult<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    };
    let base = format!("hooks.json.bak.{}", Local::now().format("%Y%m%d-%H%M%S"));
    let mut target = path.with_file_name(&base);
    let mut counter = 1;
    while target.exists() {
        target = path.with_file_name(format!("{base}.{counter}"));
        counter += 1;
    }
    std::fs::copy(path, &target)?;
    Ok(Some(target))
}
fn write(path: &Path, value: &Value) -> CoreResult<()> {
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| CoreError::HookConfig("invalid hooks path".into()))?,
    )?;
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    use std::io::Write;
    temporary.write_all(&serde_json::to_vec_pretty(value)?)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

pub fn status(binary: &Path) -> CoreResult<HookStatus> {
    let path = hooks_path();
    let doc = load(&path)?;
    let mut events = vec![];
    let mut count = 0;
    let mut current_count = 0_u32;
    let mut command = String::new();
    let stored_states = stored_hook_states();
    let mut trust_matches = 0_u32;
    let mut trust_mismatches = 0_u32;
    let mut trust_missing = 0_u32;
    let mut enabled_events = HashSet::new();
    if let Some(hooks) = doc.get("hooks").and_then(Value::as_object) {
        for (event, groups) in hooks {
            let Some(groups) = groups.as_array() else {
                continue;
            };
            for (group_index, group) in groups.iter().enumerate() {
                let Some(handlers) = group.get("hooks").and_then(Value::as_array) else {
                    continue;
                };
                for (handler_index, handler) in handlers.iter().enumerate() {
                    if ours(handler) {
                        count += 1;
                        if !events.contains(event) {
                            events.push(event.clone());
                        }
                        let configured = format!(
                            "{} {}",
                            handler.get("command").and_then(Value::as_str).unwrap_or(""),
                            handler
                                .get("commandWindows")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                        );
                        #[cfg(windows)]
                        {
                            if configured
                                .to_lowercase()
                                .contains(&binary.to_string_lossy().to_lowercase())
                            {
                                current_count += 1;
                            }
                        }
                        #[cfg(not(windows))]
                        {
                            if configured.contains(binary.to_string_lossy().as_ref()) {
                                current_count += 1;
                            }
                        }
                        command = configured;
                        if let (Some(event_key), Some(expected_hash)) =
                            (event_key(event), handler_hash(event, handler))
                        {
                            let key = format!(
                                "{}:{event_key}:{group_index}:{handler_index}",
                                path.display()
                            );
                            match stored_states.as_ref().and_then(|states| states.get(&key)) {
                                Some(state) => {
                                    if state.enabled {
                                        enabled_events.insert(event.clone());
                                    }
                                    match state.trusted_hash.as_deref() {
                                        Some(hash) if hash == expected_hash => trust_matches += 1,
                                        Some(_) => trust_mismatches += 1,
                                        None => trust_missing += 1,
                                    }
                                }
                                None => {
                                    // Codex defaults hooks to enabled when there is no state entry.
                                    enabled_events.insert(event.clone());
                                    trust_missing += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    events.sort();
    let installed = events.iter().any(|value| value == "Stop")
        && events.iter().any(|value| value == "PermissionRequest")
        && events.iter().any(|value| value == "PreToolUse");
    let trust_status = if !installed {
        "notInstalled"
    } else if stored_states.is_none() {
        "unknown"
    } else if trust_mismatches > 0 {
        "modified"
    } else if trust_missing > 0 {
        "untrusted"
    } else if trust_matches >= 3 {
        "trusted"
    } else {
        "unknown"
    };
    Ok(HookStatus {
        hooks_path: path.display().to_string(),
        exists: path.exists(),
        installed,
        handler_count: count,
        installed_events: events,
        path_current: installed && current_count == count,
        configured_command: command,
        trusted: trust_status == "trusted",
        trust_status: trust_status.into(),
        review_required: matches!(trust_status, "untrusted" | "modified"),
        enabled: installed
            && ["PreToolUse", "PermissionRequest", "Stop"]
                .iter()
                .all(|event| enabled_events.contains(*event)),
    })
}
pub fn install(binary: &Path) -> CoreResult<(PathBuf, Option<PathBuf>)> {
    if !binary.exists() {
        return Err(CoreError::HookConfig(format!(
            "hook binary does not exist: {}",
            binary.display()
        )));
    };
    let path = hooks_path();
    let mut doc = load(&path)?;
    remove_ours(&mut doc);
    let hooks = doc.get_mut("hooks").and_then(Value::as_object_mut).unwrap();
    for event in ["Stop", "PermissionRequest"] {
        let groups = hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| CoreError::HookConfig(format!("hooks.{event} must be an array")))?;
        groups.push(json!({"hooks":[handler(binary)]}));
    }
    hooks
        .entry("PreToolUse")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| CoreError::HookConfig("hooks.PreToolUse must be an array".into()))?
        .push(json!({
            "matcher": "^(request_user_input|requestUserInput)$",
            "hooks": [handler(binary)]
        }));
    let copy = backup(&path)?;
    write(&path, &doc)?;
    Ok((path, copy))
}
pub fn uninstall() -> CoreResult<(PathBuf, Option<PathBuf>, u32)> {
    let path = hooks_path();
    if !path.exists() {
        return Ok((path, None, 0));
    };
    let mut doc = load(&path)?;
    let removed = remove_ours(&mut doc);
    if removed == 0 {
        return Ok((path, None, 0));
    };
    let copy = backup(&path)?;
    write(&path, &doc)?;
    Ok((path, copy, removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_command_is_independent_of_the_outer_shell() {
        assert_eq!(
            windows_command(r"C:\Users\A User\hook.exe"),
            r#"powershell.exe -NoLogo -NoProfile -NonInteractive -Command "& 'C:\Users\A User\hook.exe' --codex-notify-hook""#
        );
        assert_eq!(
            windows_command(r"D:\it's\hook.exe"),
            r#"powershell.exe -NoLogo -NoProfile -NonInteractive -Command "& 'D:\it''s\hook.exe' --codex-notify-hook""#
        );
    }
    #[test]
    fn install_preserves_other_hooks() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("codex");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            home.join("hooks.json"),
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"other"}]}]}}"#,
        )
        .unwrap();
        let bin = temp
            .path()
            .join(if cfg!(windows) { "hook.exe" } else { "hook" });
        std::fs::write(&bin, b"x").unwrap();
        std::env::set_var("CODEX_HOME", &home);
        install(&bin).unwrap();
        install(&bin).unwrap();
        let s = status(&bin).unwrap();
        assert_eq!(s.handler_count, 3);
        assert_eq!(s.trust_status, "untrusted");
        assert!(s.review_required);

        let document = load(&home.join("hooks.json")).unwrap();
        let permission = &document["hooks"]["PermissionRequest"][0]["hooks"][0];
        let pre_tool = &document["hooks"]["PreToolUse"][0]["hooks"][0];
        assert_eq!(
            document["hooks"]["PreToolUse"][0]["matcher"],
            "^(request_user_input|requestUserInput)$"
        );
        let stop = &document["hooks"]["Stop"][1]["hooks"][0];
        assert!(permission["commandWindows"]
            .as_str()
            .is_some_and(|command| command.starts_with("powershell.exe ")
                && command.contains("& '")
                && command.ends_with("\"")));
        let permission_hash = handler_hash("PermissionRequest", permission).unwrap();
        let pre_tool_hash = handler_hash("PreToolUse", pre_tool).unwrap();
        let stop_hash = handler_hash("Stop", stop).unwrap();
        let hooks_file = home.join("hooks.json").display().to_string();
        let mut states = toml::map::Map::new();
        for (key, hash) in [
            (
                format!("{hooks_file}:permission_request:0:0"),
                permission_hash,
            ),
            (format!("{hooks_file}:pre_tool_use:0:0"), pre_tool_hash),
            (format!("{hooks_file}:stop:1:0"), stop_hash),
        ] {
            let mut state = toml::map::Map::new();
            state.insert("trusted_hash".into(), toml::Value::String(hash));
            states.insert(key, toml::Value::Table(state));
        }
        let mut hooks = toml::map::Map::new();
        hooks.insert("state".into(), toml::Value::Table(states));
        let mut config = toml::map::Map::new();
        config.insert("hooks".into(), toml::Value::Table(hooks));
        std::fs::write(
            home.join("config.toml"),
            toml::to_string(&toml::Value::Table(config)).unwrap(),
        )
        .unwrap();
        let trusted = status(&bin).unwrap();
        assert!(trusted.trusted);
        assert!(trusted.enabled);
        assert_eq!(trusted.trust_status, "trusted");
        uninstall().unwrap();
        let text = std::fs::read_to_string(home.join("hooks.json")).unwrap();
        assert!(text.contains("other"));
        assert!(!text.contains(NEW_MARKER));
        std::env::remove_var("CODEX_HOME");
    }
}
