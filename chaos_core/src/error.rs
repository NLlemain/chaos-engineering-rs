use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChaosError {
    #[error("Target not found: {0}")]
    TargetNotFound(String),

    #[error("Injection failed: {0}")]
    InjectionFailed(String),

    #[error("Cleanup failed: {0}")]
    CleanupFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("System error: {0}")]
    SystemError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, ChaosError>;
