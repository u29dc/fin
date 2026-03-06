mod cli;
mod runtime;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};

fn main() {
    if let Err(error) = run() {
        eprintln!("fin-api: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    run_with(Cli::parse())
}

fn run_with(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Start(args) => {
            let plan = runtime::prepare_bootstrap(&args)?;
            eprintln!("{}", plan.status_line());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::run_with;
    use crate::cli::{Cli, Command, StartArgs};

    #[test]
    fn placeholder_start_exits_cleanly_without_runtime_check() {
        let temp = tempdir().expect("tempdir");
        let result = run_with(Cli {
            command: Command::Start(StartArgs {
                config_path: None,
                db_path: None,
                socket_path: Some(temp.path().join("fin-api.sock")),
                check_runtime: false,
            }),
        });

        assert!(result.is_ok());
    }
}
