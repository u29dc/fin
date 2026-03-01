mod commands;
mod envelope;
mod error;
mod registry;

use std::io::{self, Write};
use std::process::Command as ProcessCommand;
use std::time::Instant;

use clap::{Args, Parser, Subcommand};
use fin_sdk::SDK_VERSION;

use crate::commands::{CommandFailure, CommandResult, GlobalOptions};
use crate::envelope::{emit_error, emit_success, print_text_error};
use crate::error::ExitCode;

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
    /// Rules file management commands
    Rules(RulesArgs),
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

#[derive(Args, Debug)]
struct RulesArgs {
    #[command(subcommand)]
    command: RulesCommand,
}

#[derive(Subcommand, Debug)]
enum RulesCommand {
    /// Show merged rules metadata
    Show(RulesPathArgs),
    /// Validate TOML rules file
    Validate(RulesPathArgs),
    /// Migrate legacy TypeScript rules to TOML
    MigrateTs(RulesMigrateArgs),
}

#[derive(Args, Debug)]
struct RulesPathArgs {
    /// Override rules file path
    #[arg(long)]
    path: Option<String>,
}

#[derive(Args, Debug)]
struct RulesMigrateArgs {
    /// Source TypeScript rules file path (default: $FIN_HOME/data/fin.rules.ts)
    #[arg(long)]
    source: Option<String>,
    /// Target TOML rules file path (default: $FIN_HOME/data/fin.rules.toml)
    #[arg(long)]
    target: Option<String>,
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
        Some(Command::Rules(rules)) => match rules.command {
            RulesCommand::Show(args) => commands::rules::run_show(args.path.as_deref()),
            RulesCommand::Validate(args) => commands::rules::run_validate(args.path.as_deref()),
            RulesCommand::MigrateTs(args) => {
                commands::rules::run_migrate_ts(args.source.as_deref(), args.target.as_deref())
            }
        },
    }
}

fn first_command_token(raw_args: &[String]) -> Option<&str> {
    if raw_args.len() <= 1 {
        return None;
    }

    let mut idx = 1usize;
    while idx < raw_args.len() {
        let token = raw_args[idx].as_str();
        if token == "--db" || token == "--format" {
            idx += 2;
            continue;
        }
        if token.starts_with('-') {
            idx += 1;
            continue;
        }
        return Some(token);
    }

    None
}

fn should_delegate_to_legacy(raw_args: &[String]) -> bool {
    let Some(command) = first_command_token(raw_args) else {
        return false;
    };
    !matches!(command, "version" | "rules")
}

fn delegate_to_legacy(args: &[String]) -> i32 {
    let output = ProcessCommand::new("bun")
        .arg("run")
        .arg("packages/cli/src/index.ts")
        .args(args)
        .output();

    match output {
        Ok(output) => {
            if let Err(error) = io::stdout().write_all(&output.stdout) {
                eprintln!("failed to write delegated stdout: {error}");
                return ExitCode::Runtime.as_i32();
            }
            if let Err(error) = io::stderr().write_all(&output.stderr) {
                eprintln!("failed to write delegated stderr: {error}");
                return ExitCode::Runtime.as_i32();
            }
            output
                .status
                .code()
                .unwrap_or_else(|| ExitCode::Runtime.as_i32())
        }
        Err(error) => {
            eprintln!("failed to execute delegated legacy CLI via bun: {error}");
            ExitCode::Runtime.as_i32()
        }
    }
}

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    if should_delegate_to_legacy(&raw_args) {
        std::process::exit(delegate_to_legacy(&raw_args[1..]));
    }

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
