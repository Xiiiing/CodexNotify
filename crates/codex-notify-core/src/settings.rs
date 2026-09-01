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
    pub bark_server: String,
    pub group: String,
    pub level: String,
    pub sound: String,
    pub scope: String,
    pub projects: Vec<ProjectRule>,
    pub message_mode: String,
    pub fixed_message: String,
    pub notification_title: String,
    pub permission_notifications: bool,
    pub redact_sensitive: bool,
    pub quiet_hours_enabled: bool,
    pub quiet_start: String,
    pub quiet_end: String,
    pub quiet_action: String,
    pub bark_icon: String,
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
            schema_version: 1,
            enabled: true,
            bark_server: "https://api.day.app".into(),
            group: "Codex".into(),
            level: "active".into(),
            sound: String::new(),
            scope: "all".into(),
            projects: vec![],
            message_mode: "summary200".into(),
            fixed_message: "Codex has finished a turn. Return to your computer to view the result."
                .into(),
            notification_title: "{project}".into(),
            permission_notifications: true,
            redact_sensitive: true,
            quiet_hours_enabled: false,
            quiet_start: "22:00".into(),
            quiet_end: "08:00".into(),
            quiet_action: "silent".into(),
            bark_icon: String::new(),
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
        let value: Self = serde_json::from_slice(&std::fs::read(path)?)?;
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
        if self.schema_version != 1 {
            return Err(CoreError::InvalidConfig(
                "unsupported settings schema".into(),
            ));
        }
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
        Ok(())
    }
}
