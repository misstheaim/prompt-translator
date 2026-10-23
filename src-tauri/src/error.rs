//! Shared, serializable error type (plan §4.6).
//!
//! On the wire an error is `{ kind, message, hint, detail? }`; `kind` is the variant name.

use serde::{Serialize, Serializer};
use ts_rs::TS;

/// Machine-readable error kind, the `kind` field of the serialized error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub enum ErrorKind {
    OllamaUnreachable,
    ModelMissing,
    ClaudeNotFound,
    ClaudeAuth,
    Timeout,
    Cancelled,
    EmptyOutput,
    MalformedOutput,
    ProcessCrashed,
    SessionClosed,
    SessionUnreachable,
    InjectionFailed,
    PrefixNeedsConfirmation,
    Io,
    Db,
    InvalidInput,
}

/// The JSON shape of a serialized [`AppError`].
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppErrorPayload {
    pub kind: ErrorKind,
    pub message: String,
    pub hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub detail: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Can't reach Ollama")]
    OllamaUnreachable,
    #[error("Model \"{0}\" is not installed")]
    ModelMissing(String),
    #[error("claude.exe was not found")]
    ClaudeNotFound,
    #[error("Claude Code is not signed in")]
    ClaudeAuth,
    #[error("The request timed out")]
    Timeout,
    #[error("The request was cancelled")]
    Cancelled,
    #[error("The model returned an empty result")]
    EmptyOutput,
    /// Carries the raw model text for debugging (serialized as `detail`).
    #[error("The model returned an unreadable result")]
    MalformedOutput(String),
    /// Carries a stderr excerpt (serialized as `detail`).
    #[error("The provider process crashed")]
    ProcessCrashed(String),
    #[error("The bound Claude session has ended")]
    SessionClosed,
    #[error("Can't write to the Claude session")]
    SessionUnreachable,
    #[error("Sending to the Claude session failed")]
    InjectionFailed,
    #[error("The text starts with a special character")]
    PrefixNeedsConfirmation,
    #[error("File error: {0}")]
    Io(String),
    #[error("Database error: {0}")]
    Db(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::OllamaUnreachable => ErrorKind::OllamaUnreachable,
            Self::ModelMissing(_) => ErrorKind::ModelMissing,
            Self::ClaudeNotFound => ErrorKind::ClaudeNotFound,
            Self::ClaudeAuth => ErrorKind::ClaudeAuth,
            Self::Timeout => ErrorKind::Timeout,
            Self::Cancelled => ErrorKind::Cancelled,
            Self::EmptyOutput => ErrorKind::EmptyOutput,
            Self::MalformedOutput(_) => ErrorKind::MalformedOutput,
            Self::ProcessCrashed(_) => ErrorKind::ProcessCrashed,
            Self::SessionClosed => ErrorKind::SessionClosed,
            Self::SessionUnreachable => ErrorKind::SessionUnreachable,
            Self::InjectionFailed => ErrorKind::InjectionFailed,
            Self::PrefixNeedsConfirmation => ErrorKind::PrefixNeedsConfirmation,
            Self::Io(_) => ErrorKind::Io,
            Self::Db(_) => ErrorKind::Db,
            Self::InvalidInput(_) => ErrorKind::InvalidInput,
        }
    }

    /// The hint shown in the UI (plan §4.6).
    pub fn hint(&self) -> String {
        match self {
            Self::OllamaUnreachable => "Ollama isn't running.".into(),
            Self::ModelMissing(model) => {
                format!("Run `ollama pull {model}` or pick another model.")
            }
            Self::ClaudeNotFound => "Set the claude.exe path in Settings.".into(),
            Self::ClaudeAuth => {
                "Claude Code isn't signed in. Run `claude` once and sign in.".into()
            }
            Self::Timeout | Self::Cancelled => "Retry.".into(),
            Self::EmptyOutput | Self::MalformedOutput(_) => {
                "Retry, or try another provider.".into()
            }
            Self::ProcessCrashed(_) => "See the error details.".into(),
            Self::SessionClosed => "The bound Claude session has ended. Pick another session \
                                    or unbind to copy instead."
                .into(),
            Self::SessionUnreachable => {
                "Can't write to this session. Is its terminal running as administrator?".into()
            }
            Self::InjectionFailed => "Sending failed. The prompt was copied to the clipboard \
                                      instead; paste it manually."
                .into(),
            Self::PrefixNeedsConfirmation => "This starts with `!` / `/` / `#` and will run as a \
                                              shell command / slash command / memory note in \
                                              Claude Code. Send anyway?"
                .into(),
            Self::Io(_) | Self::Db(_) | Self::InvalidInput(_) => String::new(),
        }
    }

    fn detail(&self) -> Option<String> {
        match self {
            Self::MalformedOutput(raw) | Self::ProcessCrashed(raw) if !raw.is_empty() => {
                Some(raw.clone())
            }
            _ => None,
        }
    }

    pub fn payload(&self) -> AppErrorPayload {
        AppErrorPayload {
            kind: self.kind(),
            message: self.to_string(),
            hint: self.hint(),
            detail: self.detail(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.payload().serialize(serializer)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::InvalidInput(e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        Self::Io(e.to_string())
    }
}
