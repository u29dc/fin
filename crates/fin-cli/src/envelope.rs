use std::time::Instant;

use serde::Serialize;

use crate::error::{CliError, ExitCode};

#[derive(Debug, Clone, Serialize)]
pub struct Meta {
    pub tool: String,
    pub elapsed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct MetaExtras {
    pub count: Option<usize>,
    pub total: Option<usize>,
    pub has_more: Option<bool>,
}

#[derive(Debug, Serialize)]
struct SuccessEnvelope<T: Serialize> {
    ok: bool,
    data: T,
    meta: Meta,
}

#[derive(Debug, Serialize)]
struct ErrorMeta {
    tool: String,
    elapsed: u64,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    ok: bool,
    error: crate::error::ErrorPayload,
    meta: ErrorMeta,
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

pub fn emit_success<T: Serialize>(
    tool: &str,
    data: &T,
    start: Instant,
    extras: MetaExtras,
    exit_code: ExitCode,
) -> ExitCode {
    let envelope = SuccessEnvelope {
        ok: true,
        data,
        meta: Meta {
            tool: tool.to_string(),
            elapsed: elapsed_ms(start),
            count: extras.count,
            total: extras.total,
            has_more: extras.has_more,
        },
    };

    match serde_json::to_string(&envelope) {
        Ok(json) => {
            println!("{json}");
            exit_code
        }
        Err(error) => emit_error(
            tool,
            &CliError::new(
                crate::error::ErrorCode::Runtime,
                format!("failed to serialize success envelope: {error}"),
                "Inspect the command payload and retry with --json",
            ),
            start,
        ),
    }
}

pub fn emit_error(tool: &str, error: &CliError, start: Instant) -> ExitCode {
    let envelope = ErrorEnvelope {
        ok: false,
        error: error.payload(),
        meta: ErrorMeta {
            tool: tool.to_string(),
            elapsed: elapsed_ms(start),
        },
    };

    match serde_json::to_string(&envelope) {
        Ok(json) => println!("{json}"),
        Err(serialize_error) => {
            let fallback = format!(
                "{{\"ok\":false,\"error\":{{\"code\":\"RUNTIME_ERROR\",\"message\":\"failed to serialize error envelope: {serialize_error}\",\"hint\":\"check stderr for details\"}},\"meta\":{{\"tool\":\"{tool}\",\"elapsed\":{}}}}}",
                elapsed_ms(start)
            );
            println!("{fallback}");
        }
    }

    error.exit_code()
}

pub fn print_text_error(error: &CliError) {
    eprintln!("Error [{}]: {}", error.code.as_str(), error.message);
    if !error.hint.trim().is_empty() {
        eprintln!("Hint: {}", error.hint);
    }
}
