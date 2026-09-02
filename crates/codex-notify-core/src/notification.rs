use crate::settings::AppSettings;
use chrono::{Local, NaiveTime};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};
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
    #[serde(
        default,
        alias = "sessionName",
        deserialize_with = "deserialize_nullable_string"
    )]
    pub session_name: String,
    #[serde(
        default,
        alias = "threadName",
        deserialize_with = "deserialize_nullable_string"
    )]
    pub thread_name: String,
    #[serde(default, alias = "toolUseId")]
    pub tool_use_id: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(
        default,
        alias = "lastAssistantMessage",
        deserialize_with = "deserialize_nullable_string"
    )]
    pub last_assistant_message: String,
    #[serde(default, alias = "toolName")]
    pub tool_name: String,
    #[serde(default, alias = "toolInput")]
    pub tool_input: Value,
    #[serde(default)]
    pub diagnostic: bool,
}

fn deserialize_nullable_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub event_key: String,
    #[serde(default)]
    pub bark_id: String,
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
    #[serde(default)]
    pub markdown: bool,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub volume: Option<u8>,
    #[serde(default)]
    pub badge: Option<i64>,
    #[serde(default)]
    pub call: bool,
    #[serde(default)]
    pub auto_copy: bool,
    #[serde(default)]
    pub copy: String,
    #[serde(default)]
    pub archive: Option<bool>,
    #[serde(default)]
    pub ttl: Option<u64>,
    #[serde(default)]
    pub action: String,
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

pub fn redact_sensitive_text(text: &str) -> String {
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

fn render_template(
    template: &str,
    device: &str,
    project: &str,
    status: &str,
    icon: &str,
) -> String {
    template
        .replace("{device}", device)
        .replace("{project}", project)
        .replace("{status}", status)
        .replace("{icon}", icon)
}

fn is_user_input_request(event: &HookEvent) -> bool {
    event.hook_event_name == "PreToolUse"
        && matches!(
            event.tool_name.as_str(),
            "request_user_input" | "requestUserInput"
        )
}

fn input_request_body(event: &HookEvent) -> String {
    let mut sections = Vec::new();
    if let Some(questions) = event.tool_input.get("questions").and_then(Value::as_array) {
        for question in questions {
            let header = question
                .get("header")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let prompt = question
                .get("question")
                .or_else(|| question.get("prompt"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let mut section = if header.is_empty() {
                prompt.to_owned()
            } else if prompt.is_empty() {
                header.to_owned()
            } else {
                format!("{header}: {prompt}")
            };
            if let Some(options) = question.get("options").and_then(Value::as_array) {
                let labels = options
                    .iter()
                    .filter_map(|option| {
                        let label = option
                            .get("label")
                            .and_then(Value::as_str)
                            .or_else(|| option.as_str())?;
                        let description = option
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim();
                        Some(if description.is_empty() {
                            label.to_owned()
                        } else {
                            format!("{label} — {description}")
                        })
                    })
                    .filter(|label| !label.trim().is_empty())
                    .collect::<Vec<_>>();
                if !labels.is_empty() {
                    if !section.is_empty() {
                        section.push('\n');
                    }
                    section.push_str("• ");
                    section.push_str(&labels.join("\n• "));
                }
            }
            if !section.is_empty() {
                sections.push(section);
            }
        }
    }
    if sections.is_empty() {
        if let Some(prompt) = event
            .tool_input
            .get("question")
            .or_else(|| event.tool_input.get("prompt"))
            .and_then(Value::as_str)
        {
            sections.push(prompt.trim().to_owned());
        }
    }
    if sections.is_empty() {
        "Codex needs your input to continue.".into()
    } else {
        sections.join("\n\n")
    }
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
    if is_user_input_request(event) {
        return input_request_body(event);
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
    } else if is_user_input_request(event) {
        ("Waiting for input", "❓")
    } else {
        classify(&event.last_assistant_message)
    };
    let mut body = body_for(event, settings, status);
    if settings.redact_sensitive {
        body = redact_sensitive_text(&body);
    }
    let mut hasher = Sha256::new();
    hasher.update(event.hook_event_name.as_bytes());
    hasher.update(b"|");
    hasher.update(event.session_id.as_bytes());
    hasher.update(b"|");
    hasher.update(event.turn_id.as_bytes());
    if event.hook_event_name == "PermissionRequest" || is_user_input_request(event) {
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
        let important = event.hook_event_name == "PermissionRequest"
            || is_user_input_request(event)
            || status == "Waiting for input"
            || status == "Execution failed";
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
    let event_key = format!("{:x}", hasher.finalize());
    Notification {
        bark_id: event_key.clone(),
        event_key,
        event_type: if is_user_input_request(event) || status == "Waiting for input" {
            "UserInputRequest".into()
        } else {
            event.hook_event_name.clone()
        },
        session_id: event.session_id.clone(),
        turn_id: event.turn_id.clone(),
        project: project.clone(),
        cwd: event.cwd.clone(),
        title: format!("{} · {}", settings.device_name.trim(), project),
        subtitle: if event.session_name.trim().is_empty() {
            event.thread_name.trim().to_owned()
        } else {
            event.session_name.trim().to_owned()
        },
        body,
        group: settings.group.clone(),
        level,
        sound,
        icon: settings.bark_icon.clone(),
        url: render_template(
            &settings.click_url,
            settings.device_name.trim(),
            &project,
            status,
            icon,
        ),
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
        let notification = build_notification(&event, &AppSettings::default());
        assert!(notification.subtitle.is_empty());
        assert_eq!(notification.body, "请确认是否继续。");
    }

    #[test]
    fn accepts_nullable_official_stop_message() {
        let event: HookEvent = serde_json::from_value(serde_json::json!({
            "hook_event_name": "Stop",
            "session_id": "session",
            "turn_id": "turn",
            "cwd": "C:\\work",
            "stop_hook_active": false,
            "last_assistant_message": null
        }))
        .unwrap();
        assert!(event.last_assistant_message.is_empty());
        let notification = build_notification(&event, &AppSettings::default());
        assert!(notification.subtitle.is_empty());
        assert!(!notification.body.is_empty());
    }
    #[test]
    fn request_user_input_uses_question_content_and_hook_session_name() {
        let event: HookEvent = serde_json::from_value(serde_json::json!({
            "hook_event_name": "PreToolUse",
            "session_id": "session",
            "turn_id": "turn",
            "sessionName": "Release preparation",
            "cwd": "/work/CodexNotify",
            "tool_name": "request_user_input",
            "tool_use_id": "question-1",
            "tool_input": {
                "questions": [{
                    "question": "Which release channel should be used?",
                    "options": [{"label":"Stable"},{"label":"Beta"}]
                }]
            }
        }))
        .unwrap();
        let settings = AppSettings {
            device_name: "Studio-PC".into(),
            ..AppSettings::default()
        };
        let notification = build_notification(&event, &settings);
        assert_eq!(notification.event_type, "UserInputRequest");
        assert_eq!(notification.title, "Studio-PC · CodexNotify");
        assert_eq!(notification.subtitle, "Release preparation");
        assert!(notification.body.contains("Which release channel"));
        assert!(notification.body.contains("• Stable"));
        assert!(notification.body.contains("• Beta"));
    }
    #[test]
    fn thread_name_is_used_only_when_session_name_is_absent() {
        let event: HookEvent = serde_json::from_value(serde_json::json!({
            "hook_event_name": "Stop",
            "sessionName": "",
            "threadName": "Hook-provided thread",
            "cwd": "/work/demo",
            "last_assistant_message": "Done"
        }))
        .unwrap();
        assert_eq!(
            build_notification(&event, &AppSettings::default()).subtitle,
            "Hook-provided thread"
        );
    }
    #[test]
    fn redacts_secrets() {
        assert!(
            !redact_sensitive_text("api_key=abcdef123456 user@example.com")
                .contains("abcdef123456")
        );
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
