use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::FinError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeMeta {
    pub tool: String,
    pub elapsed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

impl EnvelopeMeta {
    #[must_use]
    pub fn new(tool: impl Into<String>, elapsed: u64) -> Self {
        Self {
            tool: tool.into(),
            elapsed,
            count: None,
            total: None,
            has_more: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    pub hint: String,
}

impl ErrorPayload {
    #[must_use]
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            hint: hint.into(),
        }
    }

    #[must_use]
    pub fn from_error(error: &FinError) -> Self {
        Self {
            code: error.code().to_owned(),
            message: error.to_string(),
            hint: error.hint().unwrap_or_default().to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessEnvelope<T: Serialize> {
    pub ok: bool,
    pub data: T,
    pub meta: EnvelopeMeta,
}

impl<T: Serialize> SuccessEnvelope<T> {
    #[must_use]
    pub fn new(data: T, meta: EnvelopeMeta) -> Self {
        Self {
            ok: true,
            data,
            meta,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub ok: bool,
    pub error: ErrorPayload,
    pub meta: EnvelopeMeta,
}

impl ErrorEnvelope {
    #[must_use]
    pub fn from_fin_error(error: &FinError, meta: EnvelopeMeta) -> Self {
        Self {
            ok: false,
            error: ErrorPayload::from_error(error),
            meta,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Envelope<T: Serialize> {
    Success(SuccessEnvelope<T>),
    Error(ErrorEnvelope),
}

impl<T: Serialize> Envelope<T> {
    #[must_use]
    pub fn success(data: T, meta: EnvelopeMeta) -> Self {
        Self::Success(SuccessEnvelope::new(data, meta))
    }

    #[must_use]
    pub fn error(error: &FinError, meta: EnvelopeMeta) -> Self {
        Self::Error(ErrorEnvelope::from_fin_error(error, meta))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputFieldSchema {
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

pub type OutputSchema = BTreeMap<String, OutputFieldSchema>;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub read_only: bool,
    pub supports_json: bool,
    pub interactive_only: bool,
    pub rate_limit: Option<String>,
    pub example: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalFlag {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy)]
struct ToolTraits {
    idempotent: bool,
    read_only: bool,
    supports_json: bool,
    interactive_only: bool,
}

impl ToolTraits {
    const fn read_only() -> Self {
        Self {
            idempotent: true,
            read_only: true,
            supports_json: true,
            interactive_only: false,
        }
    }

    const fn mutation() -> Self {
        Self {
            idempotent: false,
            read_only: false,
            supports_json: true,
            interactive_only: false,
        }
    }

    const fn interactive() -> Self {
        Self {
            idempotent: true,
            read_only: false,
            supports_json: false,
            interactive_only: true,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn tool(
    name: &str,
    command: &str,
    category: &str,
    description: &str,
    traits: ToolTraits,
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
        idempotent: traits.idempotent,
        read_only: traits.read_only,
        supports_json: traits.supports_json,
        interactive_only: traits.interactive_only,
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

#[must_use]
pub fn global_flags() -> Vec<GlobalFlag> {
    vec![
        GlobalFlag {
            name: "--text".to_string(),
            description: "Output human-readable text instead of the default JSON envelope"
                .to_string(),
        },
        GlobalFlag {
            name: "--db".to_string(),
            description: "Override database path".to_string(),
        },
        GlobalFlag {
            name: "--format".to_string(),
            description: "Text output format (table|tsv); requires --text".to_string(),
        },
    ]
}

#[must_use]
pub fn tool_registry() -> Vec<ToolMeta> {
    let mut tools = vec![
        tool(
            "health",
            "fin health",
            "system",
            "Check prerequisites and local runtime health",
            ToolTraits::read_only(),
            "fin health",
            vec![],
            &["checks", "status", "summary"],
        ),
        tool(
            "tools",
            "fin tools",
            "system",
            "Capability discovery and contract metadata",
            ToolTraits::read_only(),
            "fin tools",
            vec![ParameterMeta {
                name: "name".to_owned(),
                param_type: "string".to_owned(),
                required: false,
                description: "Tool name to inspect in detail".to_owned(),
            }],
            &["globalFlags", "tool", "tools", "version"],
        ),
        tool(
            "version",
            "fin version",
            "system",
            "Print version and sdk identity",
            ToolTraits::read_only(),
            "fin version",
            vec![],
            &["sdk", "tool"],
        ),
        tool(
            "tui.start",
            "fin start",
            "tui",
            "Launch fin terminal UI (interactive only)",
            ToolTraits::interactive(),
            "fin start",
            vec![],
            &["binary", "exitCode"],
        ),
        tool(
            "config.show",
            "fin config show",
            "config",
            "Show parsed configuration",
            ToolTraits::read_only(),
            "fin config show",
            vec![],
            &["groups", "accounts", "financial", "configPath"],
        ),
        tool(
            "config.validate",
            "fin config validate",
            "config",
            "Validate config file",
            ToolTraits::read_only(),
            "fin config validate",
            vec![],
            &["valid", "errors", "configPath"],
        ),
        tool(
            "rules.show",
            "fin rules show",
            "rules",
            "Show merged rules metadata",
            ToolTraits::read_only(),
            "fin rules show",
            vec![flag("--path", "Override rules path", false)],
            &[
                "rulesPath",
                "externalLoaded",
                "ruleCount",
                "warnOnUnmapped",
                "fallbackToRaw",
            ],
        ),
        tool(
            "rules.validate",
            "fin rules validate",
            "rules",
            "Validate JSON rules file",
            ToolTraits::read_only(),
            "fin rules validate",
            vec![flag("--path", "Override rules path", false)],
            &[
                "valid",
                "errors",
                "rulesPath",
                "externalLoaded",
                "ruleCount",
            ],
        ),
        tool(
            "rules.migrate_ts",
            "fin rules migrate-ts",
            "rules",
            "Migrate legacy TypeScript rules to JSON",
            ToolTraits::mutation(),
            "fin rules migrate-ts",
            vec![
                flag("--source", "Source fin.rules.ts path", false),
                flag("--target", "Target fin.rules.json path", false),
            ],
            &[
                "sourcePath",
                "targetPath",
                "ruleCount",
                "warnOnUnmapped",
                "fallbackToRaw",
            ],
        ),
        tool(
            "import",
            "fin import",
            "import",
            "Import transactions from inbox",
            ToolTraits::mutation(),
            "fin import",
            vec![flag("--inbox", "Override inbox directory", false)],
            &[
                "processedFiles",
                "archivedFiles",
                "skippedFiles",
                "totalTransactions",
                "uniqueTransactions",
                "duplicateTransactions",
                "journalEntriesAttempted",
                "journalEntriesCreated",
                "transferPairsCreated",
                "entryErrors",
                "accountsTouched",
                "unmappedDescriptions",
            ],
        ),
        tool(
            "sanitize.discover",
            "fin sanitize discover",
            "sanitize",
            "Discover description patterns",
            ToolTraits::read_only(),
            "fin sanitize discover --unmapped",
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
            ToolTraits::mutation(),
            "fin sanitize migrate --dry-run",
            vec![flag("--dry-run", "Preview changes only", false)],
            &["plan", "result"],
        ),
        tool(
            "sanitize.recategorize",
            "fin sanitize recategorize",
            "sanitize",
            "Recategorize uncategorized postings",
            ToolTraits::mutation(),
            "fin sanitize recategorize --dry-run",
            vec![flag("--dry-run", "Preview changes only", false)],
            &["plan", "result"],
        ),
        tool(
            "view.accounts",
            "fin view accounts",
            "view",
            "List accounts with balances",
            ToolTraits::read_only(),
            "fin view accounts --group=personal",
            vec![flag("--group", "Filter by group", false)],
            &["accounts", "total"],
        ),
        tool(
            "view.transactions",
            "fin view transactions",
            "view",
            "Query transactions",
            ToolTraits::read_only(),
            "fin view transactions --group=personal --limit=50",
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
            ToolTraits::read_only(),
            "fin view ledger --limit=50",
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
            ToolTraits::read_only(),
            "fin view balance",
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
            ToolTraits::mutation(),
            "fin view void <id>",
            vec![
                ParameterMeta {
                    name: "<id>".to_owned(),
                    param_type: "string".to_owned(),
                    required: true,
                    description: "Journal entry id".to_owned(),
                },
                flag("--dry-run", "Preview only", false),
            ],
            &[
                "originalEntryId",
                "voidEntryId",
                "postingsReversed",
                "dryRun",
            ],
        ),
        tool(
            "edit.transaction",
            "fin edit transaction",
            "edit",
            "Edit transaction description/account",
            ToolTraits::mutation(),
            "fin edit transaction <id> --description=...",
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
            &["entryId", "dryRun", "accountCreated", "changes"],
        ),
        tool(
            "report.burn",
            "fin report burn",
            "report",
            "Burn-safe outflow classification across selected groups",
            ToolTraits::read_only(),
            "fin report burn --include=business,personal,joint --months=6 --ownership-mode=user-share",
            vec![
                flag(
                    "--include",
                    "Groups csv; defaults to all configured groups",
                    false,
                ),
                flag("--months", "Months window", false),
                flag("--from", "Start date YYYY-MM-DD", false),
                flag("--to", "End date YYYY-MM-DD", false),
                flag(
                    "--include-partial-month",
                    "Include the current partial month in trailing month windows",
                    false,
                ),
                flag("--ownership-mode", "gross|user-share", false),
            ],
            &[
                "fromDate",
                "toDate",
                "requestedToDate",
                "windowMode",
                "includesPartialMonth",
                "ownershipMode",
                "groups",
                "groupTotals",
                "recurringBaseline",
                "periodicObligations",
                "nonRecurring",
                "vatPassThrough",
                "transfersExcluded",
                "periodicItems",
                "nonRecurringItems",
                "monthlySeries",
                "confidence",
            ],
        ),
        tool(
            "report.cashflow",
            "fin report cashflow",
            "report",
            "Monthly cashflow series",
            ToolTraits::read_only(),
            "fin report cashflow --group=personal --months=6",
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
            ToolTraits::read_only(),
            "fin report health --group=personal",
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
            "Historical or two-pool runway report",
            ToolTraits::read_only(),
            "fin report runway --mode=two-pool --months=12 --ownership-mode=user-share",
            vec![
                flag("--group", "Group id", false),
                flag("--consolidated", "Consolidated mode", false),
                flag("--include", "Groups csv for consolidated", false),
                flag(
                    "--months",
                    "Trailing months for two-pool burn baseline",
                    false,
                ),
                flag("--mode", "historical|two-pool", false),
                flag("--scenario", "config|tax-efficient|custom", false),
                flag(
                    "--ownership-mode",
                    "gross|user-share for two-pool mode",
                    false,
                ),
                flag(
                    "--salary-monthly-minor",
                    "Override salary draw for two-pool mode",
                    false,
                ),
                flag(
                    "--dividends-monthly-minor",
                    "Override dividend draw for two-pool mode",
                    false,
                ),
                flag(
                    "--include-joint-expenses",
                    "true|false override for whether joint recurring burn is counted in two-pool mode",
                    false,
                ),
                flag("--from", "Start date YYYY-MM-DD", false),
                flag("--to", "End date YYYY-MM-DD", false),
            ],
            &[
                "mode",
                "ownershipMode",
                "groups",
                "series",
                "latest",
                "twoPool",
            ],
        ),
        tool(
            "report.reserves",
            "fin report reserves",
            "report",
            "Reserve breakdown series",
            ToolTraits::read_only(),
            "fin report reserves --group=business",
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
            "Category breakdown or monthly median",
            ToolTraits::read_only(),
            "fin report categories --group=personal --mode=breakdown",
            vec![
                flag("--group", "Group id", true),
                flag("--mode", "breakdown|median", false),
                flag("--months", "Months window", false),
                flag("--limit", "Max rows", false),
                flag("--to", "End date YYYY-MM-DD", false),
            ],
            &["categories", "estimatedMonthly", "total"],
        ),
        tool(
            "report.audit",
            "fin report audit",
            "report",
            "Payee drill-down for expense account",
            ToolTraits::read_only(),
            "fin report audit --account=Expenses:Uncategorized",
            vec![
                flag("--account", "Expense account id", true),
                flag("--months", "Months window", false),
                flag("--limit", "Max rows", false),
                flag("--to", "End date YYYY-MM-DD", false),
            ],
            &["payees", "total"],
        ),
        tool(
            "report.summary",
            "fin report summary",
            "report",
            "Comprehensive summary payload",
            ToolTraits::read_only(),
            "fin report summary",
            vec![
                flag("--months", "Months window", false),
                flag("--to", "End date YYYY-MM-DD", false),
            ],
            &[
                "generatedAt",
                "periodMonths",
                "currency",
                "groups",
                "consolidated",
                "balanceSheet",
            ],
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::tool_registry;

    #[test]
    fn tool_registry_includes_orientation_commands() {
        let names = tool_registry()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<BTreeSet<_>>();
        assert!(names.contains("version"));
        assert!(names.contains("tools"));
        assert!(names.contains("health"));
    }

    #[test]
    fn tool_registry_names_are_unique() {
        let registry = tool_registry();
        let names = registry
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), registry.len());
    }

    #[test]
    fn tui_start_is_marked_interactive_only() {
        let tool = tool_registry()
            .into_iter()
            .find(|tool| tool.name == "tui.start")
            .expect("tui.start");
        assert!(!tool.supports_json);
        assert!(tool.interactive_only);
        assert!(!tool.read_only);
    }
}
