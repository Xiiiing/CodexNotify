use crate::error::{CoreError, CoreResult};
use directories::ProjectDirs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> CoreResult<Self> {
        if let Ok(root) = std::env::var("CODEX_NOTIFY_DATA_DIR") {
            let root = PathBuf::from(root);
            return Ok(Self {
                config_dir: root.join("config"),
                data_dir: root.join("data"),
                log_dir: root.join("logs"),
            });
        }
        let dirs = ProjectDirs::from("com", "Xiiiing", "CodexNotify").ok_or_else(|| {
            CoreError::InvalidConfig("platform application directories are unavailable".into())
        })?;
        Ok(Self {
            config_dir: dirs.config_dir().to_path_buf(),
            data_dir: dirs.data_dir().to_path_buf(),
            log_dir: dirs.data_local_dir().join("logs"),
        })
    }

    pub fn ensure(&self) -> CoreResult<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.log_dir)?;
        Ok(())
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
    pub fn events_db(&self) -> PathBuf {
        self.data_dir.join("events.sqlite3")
    }
    pub fn health_file(&self) -> PathBuf {
        self.data_dir.join("hook-health.json")
    }

    pub fn from_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            log_dir: root.join("logs"),
        }
    }
}
