use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ParameterMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputFieldSchema {
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

pub type OutputSchema = BTreeMap<String, OutputFieldSchema>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolMeta {
    pub name: String,
    pub command: String,
    pub category: String,
    pub description: String,
    pub parameters: Vec<ParameterMeta>,
    pub output_fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<OutputSchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
    pub idempotent: bool,
    pub rate_limit: Option<String>,
    pub example: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalFlag {
    pub name: String,
    pub description: String,
}

pub fn global_flags() -> Vec<GlobalFlag> {
    vec![
        GlobalFlag {
            name: "--json".to_string(),
            description: "Output as JSON envelope".to_string(),
        },
        GlobalFlag {
            name: "--db".to_string(),
            description: "Override database path".to_string(),
        },
        GlobalFlag {
            name: "--format".to_string(),
            description: "Output format (table|tsv)".to_string(),
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn tool(
    name: &str,
    command: &str,
    category: &str,
    description: &str,
    idempotent: bool,
    example: &str,
    parameters: Vec<ParameterMeta>,
    output_fields: &[&str],
) -> ToolMeta {
    ToolMeta {
        name: name.to_owned(),
        command: command.to_owned(),
        category: category.to_owned(),
        description: description.to_owned(),
        parameters,
        output_fields: output_fields
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        output_schema: None,
        input_schema: None,
        idempotent,
        rate_limit: None,
        example: example.to_owned(),
    }
}

fn flag(name: &str, description: &str, required: bool) -> ParameterMeta {
    ParameterMeta {
        name: name.to_owned(),
        param_type: "string".to_owned(),
        required,
        description: description.to_owned(),
    }
}

pub fn tool_registry() -> Vec<ToolMeta> {
    let mut tools = vec![
        tool(
            "tui.start",
            "fin start",
            "tui",
            "Launch fin terminal UI",
            true,
            "fin start",
            vec![],
            &["binary", "exitCode"],
        ),
        tool(
            "config.show",
            "fin config show",
            "config",
            "Show parsed configuration",
            true,
            "fin config show --json",
            vec![],
            &["groups", "accounts", "financial", "configPath"],
        ),
        tool(
            "config.validate",
            "fin config validate",
            "config",
            "Validate config file",
            true,
            "fin config validate --json",
            vec![],
            &["valid", "errors", "configPath"],
        ),
        tool(
            "rules.show",
            "fin rules show",
            "rules",
            "Show merged rules metadata",
            true,
            "fin rules show --json",
            vec![flag("--path", "Override rules path", false)],
            &["rulesPath", "externalLoaded", "ruleCount"],
        ),
        tool(
            "rules.validate",
            "fin rules validate",
            "rules",
            "Validate JSON rules file",
            true,
            "fin rules validate --json",
            vec![flag("--path", "Override rules path", false)],
            &["valid", "errors", "rulesPath"],
        ),
        tool(
            "rules.migrate_ts",
            "fin rules migrate-ts",
            "rules",
            "Migrate legacy TypeScript rules to JSON",
            false,
            "fin rules migrate-ts --json",
            vec![
                flag("--source", "Source fin.rules.ts path", false),
                flag("--target", "Target fin.rules.json path", false),
            ],
            &["sourcePath", "targetPath", "ruleCount"],
        ),
        tool(
            "import",
            "fin import",
            "import",
            "Import transactions from inbox",
            false,
            "fin import --json",
            vec![flag("--inbox", "Override inbox directory", false)],
            &[
                "processedFiles",
                "skippedFiles",
                "totalTransactions",
                "journalEntriesCreated",
                "archivedFiles",
            ],
        ),
        tool(
            "sanitize.discover",
            "fin sanitize discover",
            "sanitize",
            "Discover description patterns",
            true,
            "fin sanitize discover --unmapped --json",
            vec![
                flag("--unmapped", "Only include unmapped descriptions", false),
                flag("--min", "Minimum occurrences", false),
                flag("--account", "Filter account id", false),
            ],
            &["descriptions", "count"],
        ),
        tool(
            "sanitize.migrate",
            "fin sanitize migrate",
            "sanitize",
            "Apply description sanitization rules",
            false,
            "fin sanitize migrate --dry-run --json",
            vec![flag("--dry-run", "Preview changes only", false)],
            &["plan", "result"],
        ),
        tool(
            "sanitize.recategorize",
            "fin sanitize recategorize",
            "sanitize",
            "Recategorize uncategorized postings",
            false,
            "fin sanitize recategorize --dry-run --json",
            vec![flag("--dry-run", "Preview changes only", false)],
            &["plan", "result"],
        ),
        tool(
            "view.accounts",
            "fin view accounts",
            "view",
            "List accounts with balances",
            true,
            "fin view accounts --group=personal --json",
            vec![flag("--group", "Filter by group", false)],
            &["accounts", "total"],
        ),
        tool(
            "view.transactions",
            "fin view transactions",
            "view",
            "Query transactions",
            true,
            "fin view transactions --group=personal --limit=50 --json",
            vec![
                flag("--account", "Filter account id", false),
                flag("--group", "Filter group id", false),
                flag("--from", "Start date YYYY-MM-DD", false),
                flag("--to", "End date YYYY-MM-DD", false),
                flag("--search", "Description search", false),
                flag("--limit", "Maximum rows", false),
            ],
            &["transactions", "count"],
        ),
        tool(
            "view.ledger",
            "fin view ledger",
            "view",
            "Query journal entries with postings",
            true,
            "fin view ledger --limit=50 --json",
            vec![
                flag("--account", "Filter account id", false),
                flag("--from", "Start date YYYY-MM-DD", false),
                flag("--to", "End date YYYY-MM-DD", false),
                flag("--limit", "Maximum rows", false),
            ],
            &["entries", "count", "total"],
        ),
        tool(
            "view.balance",
            "fin view balance",
            "view",
            "Show balance sheet",
            true,
            "fin view balance --json",
            vec![flag("--as-of", "As-of date YYYY-MM-DD", false)],
            &[
                "assets",
                "liabilities",
                "equity",
                "income",
                "expenses",
                "netWorth",
                "netIncome",
            ],
        ),
        tool(
            "view.void",
            "fin view void",
            "view",
            "Create reversing journal entry",
            false,
            "fin view void <id> --json",
            vec![
                ParameterMeta {
                    name: "<id>".to_owned(),
                    param_type: "string".to_owned(),
                    required: true,
                    description: "Journal entry id".to_owned(),
                },
                flag("--dry-run", "Preview only", false),
            ],
            &["originalEntryId", "voidEntryId", "postingsReversed"],
        ),
        tool(
            "edit.transaction",
            "fin edit transaction",
            "edit",
            "Edit transaction description/account",
            false,
            "fin edit transaction <id> --description=... --json",
            vec![
                ParameterMeta {
                    name: "<id>".to_owned(),
                    param_type: "string".to_owned(),
                    required: true,
                    description: "Journal entry id".to_owned(),
                },
                flag("--description", "New description", false),
                flag("--account", "New expense account id", false),
                flag("--dry-run", "Preview only", false),
            ],
            &["entryId", "changes", "accountCreated", "dryRun"],
        ),
        tool(
            "report.cashflow",
            "fin report cashflow",
            "report",
            "Monthly cashflow series",
            true,
            "fin report cashflow --group=personal --months=6 --json",
            vec![
                flag("--group", "Group id", true),
                flag("--months", "Months window", false),
                flag("--from", "Start date YYYY-MM-DD", false),
                flag("--to", "End date YYYY-MM-DD", false),
            ],
            &["series", "totals"],
        ),
        tool(
            "report.health",
            "fin report health",
            "report",
            "Financial health series",
            true,
            "fin report health --group=personal --json",
            vec![
                flag("--group", "Group id", true),
                flag("--from", "Start date YYYY-MM-DD", false),
                flag("--to", "End date YYYY-MM-DD", false),
            ],
            &["series", "latest"],
        ),
        tool(
            "report.runway",
            "fin report runway",
            "report",
            "Runway projection series",
            true,
            "fin report runway --group=personal --json",
            vec![
                flag("--group", "Group id", false),
                flag("--consolidated", "Consolidated mode", false),
                flag("--include", "Groups csv for consolidated", false),
                flag("--from", "Start date YYYY-MM-DD", false),
                flag("--to", "End date YYYY-MM-DD", false),
            ],
            &["series", "latest", "groups"],
        ),
        tool(
            "report.reserves",
            "fin report reserves",
            "report",
            "Reserve breakdown series",
            true,
            "fin report reserves --group=business --json",
            vec![
                flag("--group", "Group id", true),
                flag("--from", "Start date YYYY-MM-DD", false),
                flag("--to", "End date YYYY-MM-DD", false),
            ],
            &["series", "latest"],
        ),
        tool(
            "report.categories",
            "fin report categories",
            "report",
            "Category breakdown/median",
            true,
            "fin report categories --group=personal --mode=breakdown --json",
            vec![
                flag("--group", "Group id", true),
                flag("--mode", "breakdown|median", false),
                flag("--months", "Months window", false),
                flag("--limit", "Max rows", false),
            ],
            &["mode", "rows"],
        ),
        tool(
            "report.audit",
            "fin report audit",
            "report",
            "Payee drill-down for account",
            true,
            "fin report audit --account=Expenses:Uncategorized --json",
            vec![
                flag("--account", "Expense account id", true),
                flag("--months", "Months window", false),
                flag("--limit", "Max rows", false),
            ],
            &["account", "payees"],
        ),
        tool(
            "report.summary",
            "fin report summary",
            "report",
            "Comprehensive summary payload",
            true,
            "fin report summary --json",
            vec![flag("--months", "Months window", false)],
            &["generatedAt", "periodMonths", "groups", "consolidated"],
        ),
    ];

    tools.sort_by(|left, right| {
        let by_category = left.category.cmp(&right.category);
        if by_category == Ordering::Equal {
            left.name.cmp(&right.name)
        } else {
            by_category
        }
    });
    tools
}
