pub mod bark;
pub mod dispatcher;
pub mod error;
pub mod hook_runtime;
pub mod hooks;
pub mod notification;
pub mod paths;
pub mod security;
pub mod settings;
pub mod store;

pub use dispatcher::{dispatch_due, DispatchResult};
pub use error::{CoreError, CoreResult};
pub use hook_runtime::process_hook_input;
pub use notification::{
    build_notification, redact_sensitive_text, should_notify, HookEvent, Notification,
};
pub use paths::{AppPaths, StorageInfo, StorageMode, StorageState};
pub use settings::{AppSettings, ProjectRule};
pub use store::{EventCounts, EventRecord, EventStatus, EventStore};

use tracing_subscriber::EnvFilter;

pub const APP_ID: &str = "com.xiiiing.codex-notify";

pub fn init_logging(paths: &AppPaths, process: &str) -> CoreResult<()> {
    paths.ensure()?;
    let appender = tracing_appender::rolling::daily(&paths.log_dir, format!("{process}.log"));
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("info".parse().expect("valid directive")),
        )
        .with_ansi(false)
        .with_writer(appender)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
    Ok(())
}
