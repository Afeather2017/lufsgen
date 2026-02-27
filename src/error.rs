//! Error types for LUFS calculation library

use std::fmt;
use std::io;

/// Errors that can occur during LUFS calculation
#[derive(Debug)]
pub enum LufsError {
    /// IO error occurred
    Io(io::Error),

    /// Unsupported audio format
    UnsupportedFormat(String),

    /// Audio decoding error
    DecodeError(String),

    /// EBU R128 loudness calculation error
    EbuR128Error(String),

    /// Invalid audio data
    InvalidData(String),
}

impl fmt::Display for LufsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LufsError::Io(e) => write!(f, "IO error: {}", e),
            LufsError::UnsupportedFormat(format) => write!(f, "Unsupported format: {}", format),
            LufsError::DecodeError(msg) => write!(f, "Decode error: {}", msg),
            LufsError::EbuR128Error(msg) => write!(f, "EBU R128 error: {}", msg),
            LufsError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
        }
    }
}

impl std::error::Error for LufsError {}

impl From<io::Error> for LufsError {
    fn from(err: io::Error) -> Self {
        LufsError::Io(err)
    }
}

/// Result type alias for LUFS operations
pub type Result<T> = std::result::Result<T, LufsError>;
