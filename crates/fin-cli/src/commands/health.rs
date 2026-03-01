use std::path::PathBuf;

use serde_json::to_value;

use fin_sdk::{
    CheckStatus, HealthCheck, HealthCheckOptions, HealthStatus, Severity, run_health_checks,
};

use crate::commands::{CommandFailure, CommandResult, GlobalOptions};
use crate::envelope::MetaExtras;
use crate::error::{CliError, ErrorCode, ExitCode};

fn status_label(status: &HealthStatus) -> &'static str {
    match status {
        HealthStatus::Ready => "READY",
        HealthStatus::Degraded => "DEGRADED",
        HealthStatus::Blocked => "BLOCKED",
    }
}

fn check_icon(check: &HealthCheck) -> &'static str {
    if check.status == CheckStatus::Ok {
        "+"
    } else if check.severity == Severity::Blocking {
        "x"
    } else {
        "!"
    }
}

fn status_text(status: &CheckStatus) -> &'static str {
    match status {
        CheckStatus::Ok => "ok",
        CheckStatus::Missing => "missing",
        CheckStatus::Invalid => "invalid",
    }
}

fn render_text(report: &fin_sdk::HealthReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Health: {}", status_label(&report.status)));
    for check in &report.checks {
        let detail = check
            .detail
            .as_ref()
            .map(|detail| format!(" ({detail})"))
            .unwrap_or_default();
        lines.push(format!(
            "  {} {}: {}{}",
            check_icon(check),
            check.label,
            status_text(&check.status),
            detail
        ));
        if check.status != CheckStatus::Ok
            && let Some(fixes) = &check.fix
        {
            for fix in fixes {
                lines.push(format!("    Fix: {fix}"));
            }
        }
    }
    lines.push(format!(
        "Summary: {} ok, {} blocking, {} degraded",
        report.summary.ok, report.summary.blocking, report.summary.degraded
    ));
    lines.join("\n")
}

pub fn run(options: &GlobalOptions) -> Result<CommandResult, CommandFailure> {
    let report = run_health_checks(HealthCheckOptions {
        db_path: options.db.as_ref().map(PathBuf::from),
        ..HealthCheckOptions::default()
    });

    let data = to_value(&report).map_err(|error| CommandFailure {
        tool: "health",
        error: CliError::new(
            ErrorCode::Runtime,
            format!("failed to serialize health payload: {error}"),
            "Retry with `fin health` to inspect text output",
        ),
    })?;

    let exit_code = match report.status {
        HealthStatus::Blocked => ExitCode::Blocked,
        _ => ExitCode::Success,
    };

    Ok(CommandResult {
        tool: "health",
        data,
        text: render_text(&report),
        meta: MetaExtras::default(),
        exit_code,
    })
}
