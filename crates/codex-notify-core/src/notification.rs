use crate::settings::AppSettings;
use chrono::{Local, NaiveTime};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEvent {
    #[serde(default, alias = "hookEventName")]
    pub hook_event_name: String,
    #[serde(default, alias = "sessionId")]
    pub session_id: String,
    #[serde(default, alias = "turnId")]
    pub turn_id: String,
    #[serde(default, alias = "toolUseId")]
    pub tool_use_id: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default, alias = "lastAssistantMessage")]
    pub last_assistant_message: String,
    #[serde(default, alias = "toolName")]
    pub tool_name: String,
    #[serde(default, alias = "toolInput")]
    pub tool_input: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub event_key: String,
    pub event_type: String,
    pub session_id: String,
    pub turn_id: String,
    pub project: String,
    pub cwd: String,
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub group: String,
    pub level: String,
    pub sound: String,
    pub icon: String,
    pub url: String,
    pub suppressed: bool,
    pub suppress_reason: String,
}

fn normalize_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    #[cfg(windows)]
    {
        PathBuf::from(absolute.to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        absolute
    }
}

fn is_within(candidate: &str, root: &str) -> bool {
    normalize_path(candidate).starts_with(normalize_path(root))
}

pub fn should_notify(cwd: &str, settings: &AppSettings) -> bool {
    if !settings.enabled {
        return false;
    }
    if settings.scope == "all" {
        return true;
    }
    let matched = settings
        .projects
        .iter()
        .filter(|p| p.enabled && !p.path.is_empty())
        .any(|p| is_within(cwd, &p.path));
    if settings.scope == "include" {
        matched
    } else {
        !matched
    }
}

fn project_name(cwd: &str, settings: &AppSettings) -> String {
    let mut matches: Vec<_> = settings
        .projects
        .iter()
        .filter(|p| p.enabled && !p.path.is_empty() && is_within(cwd, &p.path))
        .collect();
    matches.sort_by_key(|p| p.path.len());
    if let Some(rule) = matches.last() {
        return if rule.name.trim().is_empty() {
            Path::new(&rule.path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into()
        } else {
            rule.name.clone()
        };
    }
    Path::new(cwd)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
        .if_empty("Unknown project")
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}
impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.into()
        } else {
            self
        }
    }
}

fn classify(message: &str) -> (&'static str, &'static str) {
    let lower = message.to_lowercase();
    let failed = [
        "任务失败",
        "无法完成",
        "执行失败",
        "被阻止",
        "failed",
        "unable to complete",
        "blocked",
    ];
    if failed.iter().any(|v| lower.contains(v)) {
        return ("Execution failed", "⚠️");
    }
    let waiting = [
        "需要你确认",
        "需要你选择",
        "需要你提供",
        "请确认",
        "请选择",
        "请提供",
        "请告诉我",
        "等待你的",
        "need you to",
        "please confirm",
        "please choose",
        "please provide",
        "waiting for",
    ];
    if waiting.iter().any(|v| lower.contains(v)) {
        return ("Waiting for input", "❓");
    }
    ("Turn completed", "✅")
}

fn redact(text: &str) -> String {
    let patterns = [
        (
            r"(?i)\b(?:sk|rk|pk)-[A-Za-z0-9_-]{12,}\b",
            "[redacted API key]",
        ),
        (
            r"(?i)(?:token|api[_ -]?key|secret|password)\s*[:=]\s*[^\s,;]{6,}",
            "sensitive-field=[redacted]",
        ),
        (r"https?://[^\s?]+\?[^\s]+", "[redacted parameterized URL]"),
        (
            r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b",
            "[redacted email]",
        ),
    ];
    patterns
        .into_iter()
        .fold(text.to_string(), |value, (pattern, replacement)| {
            Regex::new(pattern)
                .unwrap()
                .replace_all(&value, replacement)
                .into_owned()
        })
}

fn quiet_now(settings: &AppSettings) -> bool {
    if !settings.quiet_hours_enabled {
        return false;
    }
    let parse = |value: &str| NaiveTime::parse_from_str(value, "%H:%M").ok();
    let (Some(start), Some(end)) = (parse(&settings.quiet_start), parse(&settings.quiet_end))
    else {
        return false;
    };
    quiet_at(settings, Local::now().time(), start, end)
}

fn quiet_at(settings: &AppSettings, now: NaiveTime, start: NaiveTime, end: NaiveTime) -> bool {
    if !settings.quiet_hours_enabled {
        return false;
    }
    if start < end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

fn render_template(template: &str, project: &str, status: &str, icon: &str) -> String {
    template
        .replace("{project}", project)
        .replace("{status}", status)
        .replace("{icon}", icon)
}

fn permission_body(event: &HookEvent, minimal: bool) -> String {
    let tool = if event.tool_name.is_empty() {
        "Local operation"
    } else {
        &event.tool_name
    };
    if minimal {
        return format!("Codex requests approval: {tool}.");
    }
    let detail = event
        .tool_input
        .get("description")
        .or_else(|| event.tool_input.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if detail.is_empty() {
        format!("Request: {tool}\nReturn to your computer to review it.")
    } else {
        format!(
            "Request: {tool}\n{}",
            detail.chars().take(240).collect::<String>()
        )
    }
}

fn body_for(event: &HookEvent, settings: &AppSettings, status: &str) -> String {
    if event.hook_event_name == "PermissionRequest" {
        return permission_body(event, settings.message_mode == "minimal");
    }
    if settings.message_mode == "minimal" {
        return format!("Codex status: {status}.");
    }
    if settings.message_mode == "fixed" {
        return settings.fixed_message.clone();
    }
    let compact = event
        .last_assistant_message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .if_empty(&format!(
            "Codex status: {status}. Return to your computer for details."
        ));
    let limit = match settings.message_mode.as_str() {
        "summary200" => Some(200),
        "summary500" => Some(500),
        _ => None,
    };
    if let Some(limit) = limit {
        if compact.chars().count() > limit {
            return compact.chars().take(limit - 1).collect::<String>() + "…";
        }
    }
    compact
}

pub fn build_notification(event: &HookEvent, settings: &AppSettings) -> Notification {
    let project = project_name(&event.cwd, settings);
    let (status, icon) = if event.hook_event_name == "PermissionRequest" {
        ("Waiting for approval", "🔐")
    } else {
        classify(&event.last_assistant_message)
    };
    let mut body = body_for(event, settings, status);
    if settings.redact_sensitive {
        body = redact(&body);
    }
    let mut hasher = Sha256::new();
    hasher.update(event.hook_event_name.as_bytes());
    hasher.update(b"|");
    hasher.update(event.session_id.as_bytes());
    hasher.update(b"|");
    hasher.update(event.turn_id.as_bytes());
    if event.hook_event_name == "PermissionRequest" {
        hasher.update(b"|");
        if event.tool_use_id.is_empty() {
            hasher.update(event.tool_name.as_bytes());
            hasher.update(b"|");
            hasher.update(event.tool_input.to_string().as_bytes());
        } else {
            hasher.update(event.tool_use_id.as_bytes());
        }
    }
    let mut level = settings.level.clone();
    let mut sound = settings.sound.clone();
    let mut suppressed = false;
    let mut suppress_reason = String::new();
    if quiet_now(settings) {
        let important =
            event.hook_event_name == "PermissionRequest" || status == "Execution failed";
        if settings.quiet_action == "pause"
            || (settings.quiet_action == "importantOnly" && !important)
        {
            suppressed = true;
            suppress_reason = "quietHours".into();
        } else if settings.quiet_action == "silent" {
            level = "passive".into();
            sound.clear();
        }
    }
    Notification {
        event_key: format!("{:x}", hasher.finalize()),
        event_type: event.hook_event_name.clone(),
        session_id: event.session_id.clone(),
        turn_id: event.turn_id.clone(),
        project: project.clone(),
        cwd: event.cwd.clone(),
        title: format!(
            "{icon} {}",
            render_template(&settings.notification_title, &project, status, icon)
        ),
        subtitle: status.into(),
        body,
        group: settings.group.clone(),
        level,
        sound,
        icon: settings.bark_icon.clone(),
        url: render_template(&settings.click_url, &project, status, icon),
        suppressed,
        suppress_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifies_and_truncates() {
        let s = AppSettings {
            message_mode: "summary200".into(),
            ..AppSettings::default()
        };
        let e = HookEvent {
            hook_event_name: "Stop".into(),
            last_assistant_message: "a".repeat(250),
            ..serde_json::from_value(serde_json::json!({})).unwrap()
        };
        let n = build_notification(&e, &s);
        assert_eq!(n.body.chars().count(), 200);
        assert!(n.body.ends_with('…'));
    }

    #[test]
    fn accepts_official_snake_case_hook_input() {
        let event: HookEvent = serde_json::from_value(serde_json::json!({
            "hook_event_name":"Stop", "session_id":"session", "turn_id":"turn",
            "cwd":"/work/画图", "last_assistant_message":"请确认是否继续。"
        }))
        .unwrap();
        assert_eq!(event.hook_event_name, "Stop");
        assert_eq!(
            build_notification(&event, &AppSettings::default()).subtitle,
            "Waiting for input"
        );
    }
    #[test]
    fn redacts_secrets() {
        assert!(!redact("api_key=abcdef123456 user@example.com").contains("abcdef123456"));
    }
    #[test]
    fn permission_is_unique_by_tool() {
        let s = AppSettings::default();
        let mut e: HookEvent = serde_json::from_value(serde_json::json!({"hookEventName":"PermissionRequest","sessionId":"s","turnId":"t","toolUseId":"a"})).unwrap();
        let a = build_notification(&e, &s).event_key;
        e.tool_use_id = "b".into();
        assert_ne!(a, build_notification(&e, &s).event_key);
    }

    #[test]
    fn quiet_hours_cross_midnight() {
        let settings = AppSettings {
            quiet_hours_enabled: true,
            ..AppSettings::default()
        };
        let start = NaiveTime::from_hms_opt(22, 0, 0).unwrap();
        let end = NaiveTime::from_hms_opt(8, 0, 0).unwrap();
        assert!(quiet_at(
            &settings,
            NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            start,
            end
        ));
        assert!(quiet_at(
            &settings,
            NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
            start,
            end
        ));
        assert!(!quiet_at(
            &settings,
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            start,
            end
        ));
    }

    #[test]
    fn project_scope_matches_children() {
        let settings = AppSettings {
            scope: "include".into(),
            projects: vec![crate::ProjectRule {
                path: "/work/demo".into(),
                name: "Demo".into(),
                enabled: true,
            }],
            ..AppSettings::default()
        };
        assert!(should_notify("/work/demo/src", &settings));
        assert!(!should_notify("/work/other", &settings));
    }
}
