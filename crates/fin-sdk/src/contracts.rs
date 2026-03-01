use serde::{Deserialize, Serialize};

use crate::error::FinError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeMeta {
    pub tool: String,
    pub elapsed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

impl EnvelopeMeta {
    #[must_use]
    pub fn new(tool: impl Into<String>, elapsed: u64) -> Self {
        Self {
            tool: tool.into(),
            elapsed,
            count: None,
            total: None,
            has_more: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl ErrorPayload {
    #[must_use]
    pub fn from_error(error: &FinError) -> Self {
        Self {
            code: error.code().to_owned(),
            message: error.to_string(),
            hint: error.hint().map(std::string::ToString::to_string),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessEnvelope<T: Serialize> {
    pub ok: bool,
    pub data: T,
    pub meta: EnvelopeMeta,
}

impl<T: Serialize> SuccessEnvelope<T> {
    #[must_use]
    pub fn new(data: T, meta: EnvelopeMeta) -> Self {
        Self {
            ok: true,
            data,
            meta,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub error: ErrorPayload,
    pub meta: EnvelopeMeta,
}

impl ErrorEnvelope {
    #[must_use]
    pub fn from_fin_error(error: &FinError, meta: EnvelopeMeta) -> Self {
        Self {
            ok: false,
            error: ErrorPayload::from_error(error),
            meta,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Envelope<T: Serialize> {
    Success(SuccessEnvelope<T>),
    Error(ErrorEnvelope),
}

impl<T: Serialize> Envelope<T> {
    #[must_use]
    pub fn success(data: T, meta: EnvelopeMeta) -> Self {
        Self::Success(SuccessEnvelope::new(data, meta))
    }

    #[must_use]
    pub fn error(error: &FinError, meta: EnvelopeMeta) -> Self {
        Self::Error(ErrorEnvelope::from_fin_error(error, meta))
    }
}
