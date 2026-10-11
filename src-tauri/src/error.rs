//! Serializable application errors for Tauri IPC and internal `Result` paths.

use serde::Serialize;

/// Crate-level error type. Serialized as a string across the Tauri IPC boundary.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Filesystem failure.
    #[error("IO error: {0}")]
    Io(String),
    /// JSON serialize/deserialize failure.
    #[error("JSON error: {0}")]
    Json(String),
    /// SQLite or other database failure.
    #[error("Database error: {0}")]
    Database(String),
    /// External process or shell command failure.
    #[error("Command error: {0}")]
    Command(String),
    /// Requested resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),
    /// Caller supplied an invalid argument.
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    /// Unexpected internal failure.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Json(e.to_string())
    }
}

impl From<AppError> for String {
    fn from(e: AppError) -> Self {
        e.to_string()
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_display_should_include_variant_prefix() {
        let err = AppError::InvalidInput("bad font".to_string());
        assert_eq!(err.to_string(), "Invalid input: bad font");
    }

    #[test]
    fn app_error_should_serialize_as_string_for_ipc() {
        let err = AppError::NotFound("settings.json".to_string());
        let json = serde_json::to_string(&err).expect("serialize AppError");
        assert_eq!(json, "\"Not found: settings.json\"");
    }
}
