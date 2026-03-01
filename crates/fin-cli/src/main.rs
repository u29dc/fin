mod commands;
mod envelope;
mod error;
mod registry;

use std::time::Instant;

use clap::{Args, Parser, Subcommand};
use fin_sdk::SDK_VERSION;

use crate::commands::{CommandFailure, CommandResult, GlobalOptions};
use crate::envelope::{emit_error, emit_success, print_text_error};

#[derive(Parser, Debug)]
#[command(name = "fin", version = SDK_VERSION, about = "fin rust cli")]
struct Cli {
    #[arg(long, global = true, help = "Output as JSON envelope")]
    json: bool,
    #[arg(long, global = true, help = "Override database path")]
    db: Option<String>,
    #[arg(long, global = true, help = "Output format (table|tsv)")]
    format: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print version and sdk information
    Version,
    /// Capability discovery from the tool registry
    Tools(ToolsArgs),
    /// Check prerequisites and system health
    Health,
    /// Configuration commands
    Config(ConfigArgs),
}

#[derive(Args, Debug)]
struct ToolsArgs {
    /// Tool name to show detail for (e.g. config.show)
    name: Option<String>,
}

#[derive(Args, Debug)]
struct ConfigArgs {
    #[command(subcommand)]
    command: ConfigCommand,
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Show parsed configuration
    Show,
    /// Validate config file
    Validate,
}

fn execute(
    command: Option<Command>,
    options: &GlobalOptions,
) -> Result<CommandResult, CommandFailure> {
    match command {
        Some(Command::Version) | None => Ok(commands::version::run()),
        Some(Command::Tools(args)) => commands::tools::run(args.name.as_deref()),
        Some(Command::Health) => commands::health::run(options),
        Some(Command::Config(config)) => match config.command {
            ConfigCommand::Show => commands::config::run_show(),
            ConfigCommand::Validate => commands::config::run_validate(),
        },
    }
}

fn main() {
    let cli = Cli::parse();
    let options = GlobalOptions {
        db: cli.db.clone(),
        format: cli.format.clone(),
    };
    let start = Instant::now();

    let result = execute(cli.command, &options);
    let exit_code = if cli.json {
        match result {
            Ok(command) => emit_success(
                command.tool,
                &command.data,
                start,
                command.meta,
                command.exit_code,
            ),
            Err(failure) => emit_error(failure.tool, &failure.error, start),
        }
    } else {
        match result {
            Ok(command) => {
                if !command.text.trim().is_empty() {
                    eprintln!("{}", command.text);
                }
                command.exit_code
            }
            Err(failure) => {
                print_text_error(&failure.error);
                failure.error.exit_code()
            }
        }
    };

    std::process::exit(exit_code.as_i32());
}
