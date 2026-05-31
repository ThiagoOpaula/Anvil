//! Crate-wide error type and result alias.

use thiserror::Error;

/// All errors this crate can produce.
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Modrinth API returned {status}: {body}")]
    Api { status: u16, body: String },

    #[error("mod not found on Modrinth for hash {hash}")]
    HashNotFound { hash: String },

    #[error("no update available for {slug} targeting game version {game_version}")]
    Unavailable { slug: String, game_version: String },

    #[error("SHA1 verification failed: expected {expected}, got {actual}")]
    Sha1Mismatch { expected: String, actual: String },

    #[error("config error: {0}")]
    Config(String),

    #[error("download failed for {url}: {reason}")]
    Download { url: String, reason: String },

    #[error("no backups found")]
    NoBackups,

    #[error("user cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;
