use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use serde_json::json;

use crate::commands::{CommandFailure, CommandResult};
use crate::envelope::MetaExtras;
use crate::error::{CliError, ErrorCode, ExitCode};

fn tui_binary_name() -> &'static str {
    if cfg!(windows) {
        "fin-tui.exe"
    } else {
        "fin-tui"
    }
}

fn sibling_tui_binary() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    let dir = current.parent()?;
    let candidate = dir.join(tui_binary_name());
    candidate.exists().then_some(candidate)
}

fn launch_child(program: PathBuf) -> Result<i32, CommandFailure> {
    let status = ProcessCommand::new(&program)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| CommandFailure {
            tool: "tui.start",
            error: CliError::new(
                ErrorCode::Runtime,
                format!("Failed launching {}: {error}", program.display()),
                "Install workspace binaries with `bun run build`",
            ),
        })?;

    Ok(status.code().unwrap_or(1))
}

fn map_start_result(program: &Path, exit_status: i32) -> Result<CommandResult, CommandFailure> {
    if exit_status != 0 {
        return Err(CommandFailure {
            tool: "tui.start",
            error: CliError::new(
                ErrorCode::Runtime,
                format!("{} exited with status {exit_status}", program.display()),
                "Run `fin start` from an interactive terminal and inspect stderr output",
            ),
        });
    }

    let binary = program.display().to_string();
    Ok(CommandResult {
        tool: "tui.start",
        data: json!({
            "binary": binary,
            "exitCode": exit_status,
        }),
        text: format!("binary={binary} exitCode={exit_status}"),
        meta: MetaExtras::default(),
        exit_code: ExitCode::Success,
    })
}

pub fn run() -> Result<CommandResult, CommandFailure> {
    let program = sibling_tui_binary().unwrap_or_else(|| PathBuf::from(tui_binary_name()));

    let exit_status = launch_child(program.clone())?;
    map_start_result(&program, exit_status)
}

#[cfg(test)]
mod tests {
    use super::map_start_result;
    use crate::error::ExitCode;

    #[test]
    fn non_zero_tui_exit_is_reported_as_error() {
        let result = map_start_result(std::path::Path::new("fin-tui"), 1);
        let failure = result.expect_err("expected failure");
        assert_eq!(failure.tool, "tui.start");
        assert!(failure.error.message.contains("exited with status 1"));
    }

    #[test]
    fn zero_tui_exit_is_success_result() {
        let result = map_start_result(std::path::Path::new("fin-tui"), 0);
        let command = result.expect("expected success");
        assert_eq!(command.tool, "tui.start");
        assert_eq!(command.exit_code, ExitCode::Success);
        assert_eq!(command.text, "binary=fin-tui exitCode=0");
        assert_eq!(command.data["binary"], "fin-tui");
        assert_eq!(command.data["exitCode"], 0);
    }
}
