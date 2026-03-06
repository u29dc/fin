use std::time::Instant;

use serde::Serialize;

use fin_sdk::contracts::{EnvelopeMeta, ErrorEnvelope, SuccessEnvelope};

use crate::error::{CliError, ExitCode};

#[derive(Debug, Clone, Default)]
pub struct MetaExtras {
    pub count: Option<usize>,
    pub total: Option<usize>,
    pub has_more: Option<bool>,
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
    let mut meta = EnvelopeMeta::new(tool, elapsed_ms(start));
    meta.count = extras.count;
    meta.total = extras.total;
    meta.has_more = extras.has_more;
    let envelope = SuccessEnvelope::new(data, meta);

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
        meta: EnvelopeMeta::new(tool, elapsed_ms(start)),
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
