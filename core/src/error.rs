//! Error model shared by core and server. `code` is the stable,
//! machine-readable part the UI maps to localized text; `message` is the
//! technical detail shown under "details". Contract file: orchestrator only.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    /// Windows "HEIF Image Extensions" (free, Microsoft Store).
    HeifExt,
    /// Windows "HEVC Video Extensions" (paid on most machines).
    HevcExt,
    /// libheif missing on Linux / Docker.
    Libheif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "code", content = "codec")]
pub enum ErrorCode {
    NotFound,
    Forbidden,
    Unsupported,
    MissingCodec(Codec),
    Corrupt,
    TooLarge,
    Timeout,
    Io,
}

impl ErrorCode {
    pub fn http_status(self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::Forbidden => 403,
            Self::Unsupported | Self::MissingCodec(_) => 415,
            Self::Corrupt => 422,
            Self::TooLarge => 413,
            Self::Timeout => 504,
            Self::Io => 502,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn unsupported(what: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unsupported, what)
    }
    pub fn corrupt(what: impl Into<String>) -> Self {
        Self::new(ErrorCode::Corrupt, what)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        use std::io::ErrorKind as K;
        let code = match e.kind() {
            K::NotFound => ErrorCode::NotFound,
            K::PermissionDenied => ErrorCode::Forbidden,
            K::TimedOut => ErrorCode::Timeout,
            K::UnexpectedEof | K::InvalidData => ErrorCode::Corrupt,
            _ => ErrorCode::Io,
        };
        Self::new(code, e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
