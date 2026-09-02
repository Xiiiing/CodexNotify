use crate::error::{CoreError, CoreResult};
use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRule {
    pub path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub schema_version: u32,
    pub enabled: bool,
    pub device_name: String,
    pub bark_server: String,
    pub group: String,
    pub level: String,
    pub sound: String,
    pub scope: String,
    pub projects: Vec<ProjectRule>,
    pub message_mode: String,
    pub fixed_message: String,
    pub permission_notifications: bool,
    pub user_input_notifications: bool,
    pub redact_sensitive: bool,
    pub quiet_hours_enabled: bool,
    pub quiet_start: String,
    pub quiet_end: String,
    pub quiet_action: String,
    pub bark_icon: String,
    pub bark_markdown: bool,
    pub bark_image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bark_volume: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bark_badge: Option<i64>,
    pub bark_call: bool,
    pub bark_auto_copy: bool,
    pub bark_copy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bark_archive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bark_ttl: Option<u64>,
    pub bark_action: String,
    pub click_url: String,
    pub request_timeout: u64,
    pub retry_limit: u32,
    pub encryption_enabled: bool,
    pub encryption_algorithm: String,
    pub setup_completed: bool,
    pub language: String,
    pub theme: String,
}

fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 2,
            enabled: true,
            device_name: detected_device_name(),
            bark_server: "https://api.day.app".into(),
            group: "Codex".into(),
            level: "active".into(),
            sound: String::new(),
            scope: "all".into(),
            projects: vec![],
            message_mode: "summary200".into(),
            fixed_message: "Codex has finished a turn. Return to your computer to view the result."
                .into(),
            permission_notifications: true,
            user_input_notifications: true,
            redact_sensitive: true,
            quiet_hours_enabled: false,
            quiet_start: "22:00".into(),
            quiet_end: "08:00".into(),
            quiet_action: "silent".into(),
            bark_icon: String::new(),
            bark_markdown: false,
            bark_image: String::new(),
            bark_volume: None,
            bark_badge: None,
            bark_call: false,
            bark_auto_copy: false,
            bark_copy: String::new(),
            bark_archive: None,
            bark_ttl: None,
            bark_action: String::new(),
            click_url: String::new(),
            request_timeout: 8,
            retry_limit: 5,
            encryption_enabled: false,
            encryption_algorithm: "AES-128-CBC".into(),
            setup_completed: false,
            language: "system".into(),
            theme: "system".into(),
        }
    }
}

impl AppSettings {
    pub fn load(paths: &AppPaths) -> CoreResult<Self> {
        paths.ensure()?;
        let path = paths.settings_file();
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut value: Self = serde_json::from_slice(&std::fs::read(&path)?)?;
        match value.schema_version {
            1 => {
                value.schema_version = 2;
                if value.device_name.trim().is_empty() {
                    value.device_name = detected_device_name();
                }
                value.save(paths)?;
            }
            2 => {}
            _ => {
                return Err(CoreError::InvalidConfig(
                    "unsupported settings schema".into(),
                ))
            }
        }
        value.validate()?;
        Ok(value)
    }

    pub fn save(&self, paths: &AppPaths) -> CoreResult<()> {
        self.validate()?;
        paths.ensure()?;
        let destination = paths.settings_file();
        let mut temporary = tempfile::NamedTempFile::new_in(&paths.config_dir)?;
        let file = temporary.as_file_mut();
        file.write_all(&serde_json::to_vec_pretty(self)?)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        temporary
            .persist(destination)
            .map_err(|error| error.error)?;
        Ok(())
    }

    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != 2 {
            return Err(CoreError::InvalidConfig(
                "unsupported settings schema".into(),
            ));
        }
        if self.device_name.trim().is_empty() || self.device_name.chars().count() > 100 {
            return Err(CoreError::InvalidConfig(
                "device name must contain between 1 and 100 characters".into(),
            ));
        }
        validate_url(&self.bark_server, true, "Bark server")?;
        validate_url(&self.bark_icon, true, "Bark icon")?;
        validate_url(&self.bark_image, true, "Bark image")?;
        validate_url(&self.click_url, false, "Bark click URL")?;
        if !["all", "include", "exclude"].contains(&self.scope.as_str()) {
            return Err(CoreError::InvalidConfig("invalid project scope".into()));
        }
        if !["minimal", "fixed", "summary200", "summary500", "full"]
            .contains(&self.message_mode.as_str())
        {
            return Err(CoreError::InvalidConfig("invalid message mode".into()));
        }
        if !["active", "timeSensitive", "passive", "critical"].contains(&self.level.as_str()) {
            return Err(CoreError::InvalidConfig("invalid Bark level".into()));
        }
        if !["silent", "pause", "importantOnly"].contains(&self.quiet_action.as_str()) {
            return Err(CoreError::InvalidConfig(
                "invalid quiet-hours action".into(),
            ));
        }
        if !["AES-128-CBC", "AES-256-CBC"].contains(&self.encryption_algorithm.as_str()) {
            return Err(CoreError::InvalidConfig(
                "invalid encryption algorithm".into(),
            ));
        }
        if !(2..=30).contains(&self.request_timeout) {
            return Err(CoreError::InvalidConfig(
                "request timeout must be between 2 and 30 seconds".into(),
            ));
        }
        if !(1..=8).contains(&self.retry_limit) {
            return Err(CoreError::InvalidConfig(
                "retry limit must be between 1 and 8".into(),
            ));
        }
        if self.bark_volume.is_some_and(|value| value > 10) {
            return Err(CoreError::InvalidConfig(
                "Bark critical volume must be between 0 and 10".into(),
            ));
        }
        if self.bark_ttl == Some(0) {
            return Err(CoreError::InvalidConfig(
                "Bark archive retention must be greater than zero".into(),
            ));
        }
        if !matches!(self.bark_action.as_str(), "" | "alert") {
            return Err(CoreError::InvalidConfig("invalid Bark action".into()));
        }
        Ok(())
    }
}

fn validate_url(value: &str, http_only: bool, label: &str) -> CoreResult<()> {
    if value.trim().is_empty() {
        return Ok(());
    }
    let parsed = url::Url::parse(value.trim())
        .map_err(|_| CoreError::InvalidConfig(format!("{label} is invalid")))?;
    if http_only && !matches!(parsed.scheme(), "http" | "https") {
        return Err(CoreError::InvalidConfig(format!(
            "{label} must use HTTP or HTTPS"
        )));
    }
    Ok(())
}

pub fn detected_device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|name| name.into_string().ok())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "This device".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_v1_and_persists_detected_device_name() {
        let temporary = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temporary.path());
        paths.ensure().unwrap();
        std::fs::write(
            paths.settings_file(),
            br#"{"schemaVersion":1,"enabled":true,"barkServer":"https://api.day.app","notificationTitle":"{project}"}"#,
        )
        .unwrap();
        let settings = AppSettings::load(&paths).unwrap();
        assert_eq!(settings.schema_version, 2);
        assert!(!settings.device_name.is_empty());
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(paths.settings_file()).unwrap()).unwrap();
        assert_eq!(saved["schemaVersion"], 2);
        assert!(saved["deviceName"]
            .as_str()
            .is_some_and(|name| !name.is_empty()));
        assert!(saved.get("notificationTitle").is_none());
    }
}
