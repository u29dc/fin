use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ErrorCode {
    NotFound,
    NoConfig,
    InvalidConfig,
    NoDatabase,
    SchemaMismatch,
    Runtime,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "NOT_FOUND",
            Self::NoConfig => "NO_CONFIG",
            Self::InvalidConfig => "INVALID_CONFIG",
            Self::NoDatabase => "NO_DATABASE",
            Self::SchemaMismatch => "SCHEMA_MISMATCH",
            Self::Runtime => "RUNTIME_ERROR",
        }
    }

    pub const fn is_blocking(self) -> bool {
        matches!(
            self,
            Self::NoConfig | Self::NoDatabase | Self::SchemaMismatch
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Success = 0,
    Runtime = 1,
    Blocked = 2,
}

impl ExitCode {
    pub const fn from_error(code: ErrorCode) -> Self {
        if code.is_blocking() {
            Self::Blocked
        } else {
            Self::Runtime
        }
    }

    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    pub hint: String,
}

#[derive(Debug, Clone)]
pub struct CliError {
    pub code: ErrorCode,
    pub message: String,
    pub hint: String,
}

impl CliError {
    pub fn new(code: ErrorCode, message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint: hint.into(),
        }
    }

    pub fn payload(&self) -> ErrorPayload {
        ErrorPayload {
            code: self.code.as_str().to_string(),
            message: self.message.clone(),
            hint: self.hint.clone(),
        }
    }

    pub const fn exit_code(&self) -> ExitCode {
        ExitCode::from_error(self.code)
    }
}
