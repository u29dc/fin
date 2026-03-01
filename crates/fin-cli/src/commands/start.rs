use std::path::PathBuf;
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

pub fn run() -> Result<CommandResult, CommandFailure> {
    let program = sibling_tui_binary().unwrap_or_else(|| PathBuf::from(tui_binary_name()));

    let exit_status = launch_child(program.clone())?;
    let exit_code = if exit_status == 0 {
        ExitCode::Success
    } else {
        ExitCode::Runtime
    };

    Ok(CommandResult {
        tool: "tui.start",
        data: json!({
            "binary": program.display().to_string(),
            "exitCode": exit_status,
        }),
        text: String::new(),
        meta: MetaExtras::default(),
        exit_code,
    })
}
