use serde::Serialize;
use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("I/O operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("credential store operation failed: {0}")]
    Credential(String),
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("network request failed: {0}")]
    Network(String),
    #[error("the Bark device key is invalid")]
    BarkInvalidKey,
    #[error("Bark rejected the notification")]
    BarkRejected,
    #[error("the Bark request timed out")]
    BarkTimeout,
    #[error("the Bark server could not be reached")]
    BarkUnreachable,
    #[error("the Bark server returned an error")]
    BarkServer,
    #[error("the Bark server address is invalid")]
    InvalidBarkServer,
    #[error("the Bark encryption key is invalid")]
    InvalidEncryptionKey,
    #[error("hook configuration failed: {0}")]
    HookConfig(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

impl From<CoreError> for ApiError {
    fn from(value: CoreError) -> Self {
        let code = match value {
            CoreError::Io(_) => "ioError",
            CoreError::Json(_) => "invalidJson",
            CoreError::Database(_) => "databaseError",
            CoreError::Credential(_) => "credentialError",
            CoreError::InvalidConfig(_) => "invalidConfig",
            CoreError::Network(_) => "networkError",
            CoreError::BarkInvalidKey => "barkInvalidKey",
            CoreError::BarkRejected => "barkRejected",
            CoreError::BarkTimeout => "barkTimeout",
            CoreError::BarkUnreachable => "barkUnreachable",
            CoreError::BarkServer => "barkServerError",
            CoreError::InvalidBarkServer => "invalidBarkServer",
            CoreError::InvalidEncryptionKey => "invalidEncryptionKey",
            CoreError::HookConfig(_) => "hookConfigError",
        };
        Self {
            code,
            message: value.to_string(),
        }
    }
}
