use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("invalid URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("invalid feed: {0}")]
    InvalidFeed(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("proxy unavailable: {0}")]
    ProxyUnavailable(String),
    #[error("unsafe network target: {0}")]
    UnsafeTarget(String),
    #[error("content is too large")]
    ContentTooLarge,
    #[error("release checks are unavailable: {0}")]
    ReleaseUnavailable(String),
    #[error("invalid release metadata: {0}")]
    InvalidRelease(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("credential error: {0}")]
    Credential(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML error: {0}")]
    Xml(String),
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        let code = match &error {
            AppError::Database(_) | AppError::Migration(_) => "database_error",
            AppError::Network(value)
                if value.status() == Some(reqwest::StatusCode::UNAUTHORIZED) =>
            {
                "authentication_failed"
            }
            AppError::Network(_) => "network_error",
            AppError::Url(_) | AppError::InvalidInput(_) => "invalid_input",
            AppError::InvalidFeed(_) => "invalid_feed",
            AppError::ProxyUnavailable(_) => "proxy_unavailable",
            AppError::UnsafeTarget(_) => "unsafe_redirect",
            AppError::ContentTooLarge => "content_too_large",
            AppError::ReleaseUnavailable(_) => "release_unavailable",
            AppError::InvalidRelease(_) => "invalid_release",
            AppError::NotFound(_) => "not_found",
            AppError::Credential(_) => "credential_error",
            AppError::Io(_) => "io_error",
            AppError::Xml(_) => "invalid_opml",
        };
        Self {
            code: code.into(),
            message: error.to_string(),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
