#![forbid(unsafe_code)]

mod commands;
mod envelope;
mod error;
mod registry;

use std::time::Instant;

use clap::{Args, CommandFactory, Parser, Subcommand};
use fin_sdk::SDK_VERSION;

use crate::commands::{CommandFailure, CommandResult, GlobalOptions};
use crate::envelope::{emit_error, emit_success, print_text_error};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Json,
    Text,
}

#[derive(Parser, Debug)]
#[command(name = "fin", version = SDK_VERSION, about = "fin rust cli")]
struct Cli {
    #[arg(long, global = true, help = "Output human-readable text")]
    text: bool,
    #[arg(long, global = true, help = "Override database path")]
    db: Option<String>,
    #[arg(
        long,
        global = true,
        requires = "text",
        help = "Text output format (table|tsv)"
    )]
    format: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Launch the terminal UI
    Start,
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
    /// Import inbox statements and create journal entries
    Import(ImportArgs),
    /// Sanitization commands
    Sanitize(SanitizeArgs),
    /// View accounts, transactions, ledger, and balance
    View(ViewArgs),
    /// Edit commands
    Edit(EditArgs),
    /// Financial reports
    Report(ReportArgs),
}

#[derive(Args, Debug)]
struct ToolsArgs {
    /// Tool name to show detail for (e.g. config.show)
    name: Option<String>,
}

#[derive(Args, Debug)]
struct ImportArgs {
    /// Override inbox directory
    #[arg(long)]
    inbox: Option<String>,
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
    /// Validate JSON rules file
    Validate(RulesPathArgs),
    /// Migrate legacy TypeScript rules to JSON
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
    /// Target JSON rules file path (default: $FIN_HOME/data/fin.rules.json)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args, Debug)]
struct SanitizeArgs {
    #[command(subcommand)]
    command: SanitizeCommand,
}

#[derive(Subcommand, Debug)]
enum SanitizeCommand {
    /// Find description patterns in journal entries
    Discover(SanitizeDiscoverArgs),
    /// Apply description sanitization rules
    Migrate(SanitizeApplyArgs),
    /// Reclassify uncategorized postings using rules
    Recategorize(SanitizeApplyArgs),
}

#[derive(Args, Debug)]
struct SanitizeDiscoverArgs {
    #[arg(long, default_value_t = false)]
    unmapped: bool,
    #[arg(long, default_value_t = 2)]
    min: usize,
    #[arg(long)]
    account: Option<String>,
}

#[derive(Args, Debug)]
struct SanitizeApplyArgs {
    #[arg(long = "dry-run", default_value_t = false)]
    dry_run: bool,
}

#[derive(Args, Debug)]
struct ViewArgs {
    #[command(subcommand)]
    command: ViewCommand,
}

#[derive(Subcommand, Debug)]
enum ViewCommand {
    /// List accounts with balances
    Accounts(ViewAccountsArgs),
    /// List transactions
    Transactions(ViewTransactionsArgs),
    /// List ledger entries with postings
    Ledger(ViewLedgerArgs),
    /// Show balance sheet
    Balance(ViewBalanceArgs),
    /// Create reversing journal entry for an entry id
    Void(ViewVoidArgs),
}

#[derive(Args, Debug)]
struct ViewAccountsArgs {
    #[arg(long)]
    group: Option<String>,
}

#[derive(Args, Debug)]
struct ViewTransactionsArgs {
    #[arg(long)]
    account: Option<String>,
    #[arg(long)]
    group: Option<String>,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long)]
    search: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args, Debug)]
struct ViewLedgerArgs {
    #[arg(long)]
    account: Option<String>,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args, Debug)]
struct ViewBalanceArgs {
    #[arg(long = "as-of")]
    as_of: Option<String>,
}

#[derive(Args, Debug)]
struct ViewVoidArgs {
    id: String,
    #[arg(long = "dry-run", default_value_t = false)]
    dry_run: bool,
}

#[derive(Args, Debug)]
struct EditArgs {
    #[command(subcommand)]
    command: EditCommand,
}

#[derive(Subcommand, Debug)]
enum EditCommand {
    /// Edit a journal entry description and/or expense account
    Transaction(EditTransactionArgs),
}

#[derive(Args, Debug)]
struct EditTransactionArgs {
    id: String,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    account: Option<String>,
    #[arg(long = "dry-run", default_value_t = false)]
    dry_run: bool,
}

#[derive(Args, Debug)]
struct ReportArgs {
    #[command(subcommand)]
    command: ReportCommand,
}

#[derive(Subcommand, Debug)]
enum ReportCommand {
    Cashflow(ReportCashflowArgs),
    Health(ReportHealthArgs),
    Runway(ReportRunwayArgs),
    Reserves(ReportReservesArgs),
    Categories(ReportCategoriesArgs),
    Audit(ReportAuditArgs),
    Summary(ReportSummaryArgs),
}

#[derive(Args, Debug)]
struct ReportCashflowArgs {
    #[arg(long)]
    group: String,
    #[arg(long, default_value_t = 12)]
    months: usize,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
}

#[derive(Args, Debug)]
struct ReportHealthArgs {
    #[arg(long)]
    group: String,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
}

#[derive(Args, Debug)]
struct ReportRunwayArgs {
    #[arg(long)]
    group: Option<String>,
    #[arg(long, default_value_t = false)]
    consolidated: bool,
    #[arg(long)]
    include: Option<String>,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
}

#[derive(Args, Debug)]
struct ReportReservesArgs {
    #[arg(long)]
    group: String,
    #[arg(long)]
    from: Option<String>,
    #[arg(long)]
    to: Option<String>,
}

#[derive(Args, Debug)]
struct ReportCategoriesArgs {
    #[arg(long)]
    group: String,
    #[arg(long, default_value = "breakdown")]
    mode: String,
    #[arg(long, default_value_t = 3)]
    months: usize,
    #[arg(long, default_value_t = 10)]
    limit: usize,
    #[arg(long)]
    to: Option<String>,
}

#[derive(Args, Debug)]
struct ReportAuditArgs {
    #[arg(long)]
    account: String,
    #[arg(long, default_value_t = 6)]
    months: usize,
    #[arg(long, default_value_t = 50)]
    limit: usize,
    #[arg(long)]
    to: Option<String>,
}

#[derive(Args, Debug)]
struct ReportSummaryArgs {
    #[arg(long, default_value_t = 12)]
    months: usize,
    #[arg(long)]
    to: Option<String>,
}

fn execute(
    command: Option<Command>,
    options: &GlobalOptions,
) -> Result<CommandResult, CommandFailure> {
    match command {
        Some(Command::Version) => Ok(commands::version::run()),
        Some(Command::Start) => commands::start::run(),
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
        Some(Command::Import(args)) => {
            commands::import::run(args.inbox.as_deref(), options.db.as_deref())
        }
        Some(Command::Sanitize(sanitize)) => match sanitize.command {
            SanitizeCommand::Discover(args) => commands::sanitize::run_discover(
                options.db.as_deref(),
                args.unmapped,
                args.min,
                args.account.as_deref(),
            ),
            SanitizeCommand::Migrate(args) => {
                commands::sanitize::run_migrate(options.db.as_deref(), args.dry_run)
            }
            SanitizeCommand::Recategorize(args) => {
                commands::sanitize::run_recategorize(options.db.as_deref(), args.dry_run)
            }
        },
        Some(Command::View(view)) => match view.command {
            ViewCommand::Accounts(args) => {
                commands::view::run_accounts(options.db.as_deref(), args.group.as_deref())
            }
            ViewCommand::Transactions(args) => commands::view::run_transactions(
                options.db.as_deref(),
                args.account.as_deref(),
                args.group.as_deref(),
                args.from.as_deref(),
                args.to.as_deref(),
                args.search.as_deref(),
                args.limit,
            ),
            ViewCommand::Ledger(args) => commands::view::run_ledger(
                options.db.as_deref(),
                args.account.as_deref(),
                args.from.as_deref(),
                args.to.as_deref(),
                args.limit,
            ),
            ViewCommand::Balance(args) => {
                commands::view::run_balance(options.db.as_deref(), args.as_of.as_deref())
            }
            ViewCommand::Void(args) => {
                commands::view::run_void(options.db.as_deref(), &args.id, args.dry_run)
            }
        },
        Some(Command::Edit(edit)) => match edit.command {
            EditCommand::Transaction(args) => commands::edit::run_transaction(
                options.db.as_deref(),
                &args.id,
                args.description.as_deref(),
                args.account.as_deref(),
                args.dry_run,
            ),
        },
        Some(Command::Report(report)) => match report.command {
            ReportCommand::Cashflow(args) => commands::report::run_cashflow(
                options.db.as_deref(),
                &args.group,
                args.months,
                args.from.as_deref(),
                args.to.as_deref(),
            ),
            ReportCommand::Health(args) => commands::report::run_health(
                options.db.as_deref(),
                &args.group,
                args.from.as_deref(),
                args.to.as_deref(),
            ),
            ReportCommand::Runway(args) => commands::report::run_runway(
                options.db.as_deref(),
                args.group.as_deref(),
                args.consolidated,
                args.include.as_deref(),
                args.from.as_deref(),
                args.to.as_deref(),
            ),
            ReportCommand::Reserves(args) => commands::report::run_reserves(
                options.db.as_deref(),
                &args.group,
                args.from.as_deref(),
                args.to.as_deref(),
            ),
            ReportCommand::Categories(args) => commands::report::run_categories(
                options.db.as_deref(),
                &args.group,
                &args.mode,
                args.months,
                args.limit,
                args.to.as_deref(),
            ),
            ReportCommand::Audit(args) => commands::report::run_audit(
                options.db.as_deref(),
                &args.account,
                args.months,
                args.limit,
                args.to.as_deref(),
            ),
            ReportCommand::Summary(args) => commands::report::run_summary(
                options.db.as_deref(),
                args.months,
                args.to.as_deref(),
            ),
        },
        None => unreachable!("root help path should return before dispatch"),
    }
}

fn print_root_help() {
    let mut command = Cli::command().subcommand_required(true);
    command.print_help().expect("print fin root help");
    println!();
}

fn output_mode(command: Option<&Command>, text: bool) -> OutputMode {
    if text || matches!(command, Some(Command::Start)) {
        OutputMode::Text
    } else {
        OutputMode::Json
    }
}

fn main() {
    let cli = Cli::parse();
    if cli.command.is_none() {
        print_root_help();
        return;
    }
    let mode = output_mode(cli.command.as_ref(), cli.text);
    let options = GlobalOptions {
        db: cli.db.clone(),
        format: cli.format.clone(),
    };
    let start = Instant::now();

    let result = execute(cli.command, &options);
    let exit_code = match mode {
        OutputMode::Json => match result {
            Ok(command) => emit_success(
                command.tool,
                &command.data,
                start,
                command.meta,
                command.exit_code,
            ),
            Err(failure) => emit_error(failure.tool, &failure.error, start),
        },
        OutputMode::Text => match result {
            Ok(command) => {
                if !command.text.trim().is_empty() {
                    println!("{}", command.text);
                }
                command.exit_code
            }
            Err(failure) => {
                print_text_error(&failure.error);
                failure.error.exit_code()
            }
        },
    };

    std::process::exit(exit_code.as_i32());
}
