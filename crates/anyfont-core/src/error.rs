//! Error type of the engine.
//!
//! Every fallible public function returns [`Result<T>`]. Bad font files never produce an error at the
//! catalog level: they become records flagged `corrupt` (see [`crate::FontRecord::corrupt`]) and the scan
//! goes on. Errors are reserved for I/O on the caller's own files (database, settings, export files).

use std::path::PathBuf;

/// Engine error.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// File system error, with the path involved.
    #[error("{path}: {source}")]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// SQLite error from the font database.
    #[error("font database: {0}")]
    Db(#[from] rusqlite::Error),
    /// JSON (settings) error.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// XML (import/export) error.
    #[error("xml: {0}")]
    Xml(String),
    /// A font file could not be read as a font. Carries the reason shown to the user.
    #[error("invalid font: {0}")]
    InvalidFont(String),
    /// The operation does not apply (e.g. renaming "My fonts", unknown collection id).
    #[error("{0}")]
    NotAllowed(String),
    /// A name collides with an existing collection.
    #[error("a collection named \"{0}\" already exists")]
    DuplicateName(String),
    /// The scan was cancelled through its cancellation flag.
    #[error("cancelled")]
    Cancelled,
}

/// Engine result.
pub type Result<T> = std::result::Result<T, CoreError>;

impl CoreError {
    /// Builds an [`CoreError::Io`] for `path`.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        CoreError::Io { path: path.into(), source }
    }
}
