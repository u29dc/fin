#![forbid(unsafe_code)]

pub mod allocation;
pub mod categories;
pub mod compat;
pub mod config;
pub mod contracts;
pub mod dashboard;
pub mod db;
pub mod error;
pub mod health;
pub mod import;
pub mod mutations;
pub mod queries;
pub mod reports;
pub mod rules;
pub mod runtime;
pub mod sanitize;
mod stats;
pub mod testing;
pub mod units;

pub use allocation::{
    AllocationBucket, AllocationSegment, DashboardAllocationBasis, DashboardAllocationSummary,
    GroupAllocationSnapshot, report_group_allocation, report_group_allocation_for_month,
};
pub use compat::{
    AccountSummary, ConfigShowData, ConfigValidationResult, FinSdkError, GroupMetadata,
    ValidationError, build_config_show, resolve_config_path, run_health, validate_config,
};
pub use contracts::{
    Envelope, EnvelopeMeta, ErrorEnvelope, ErrorPayload, GlobalFlag, OutputFieldSchema,
    OutputSchema, ParameterMeta, SuccessEnvelope, ToolMeta, global_flags, tool_registry,
};
pub use dashboard::{CashflowKpis, ShortTermTrend, current_reporting_month, report_cashflow_kpis};
pub use db::schema::{REQUIRED_TABLES, SCHEMA_VERSION};
pub use error::{FinError, Result};
pub use health::{
    CheckStatus, HealthCheck, HealthCheckOptions, HealthReport, HealthStatus, HealthSummary,
    Severity, run_health_checks,
};
pub use import::{ImportInboxOptions, ImportResult, SkippedFile, import_inbox};
pub use mutations::{EditTransactionPreview, VoidPreview, edit_transaction, void_entry};
pub use queries::{
    AccountBalanceRow, AuditPayeePoint, BalanceSheet, CategoryBreakdownPoint, CategoryMedianPoint,
    JournalEntryRow, LedgerQueryOptions, MonthlyCashflowPoint, PostingRow, TransactionQueryOptions,
    TransactionRow, all_group_ids, audit_payees, get_balance_sheet, group_asset_account_ids,
    group_category_breakdown, group_category_monthly_median, group_monthly_cashflow,
    ledger_entry_count, transaction_counts_by_group, view_accounts, view_ledger, view_transactions,
};
pub use reports::{
    CashflowTotals, ConsolidatedSummary, GroupSummary, HealthPoint, ReserveBreakdownPoint,
    RunwayPoint, SummaryReport, report_cashflow, report_health, report_reserves, report_runway,
    report_summary,
};
pub use runtime::{RuntimeContext, RuntimeContextOptions};
pub use sanitize::{
    DescriptionSummary, MigrationCandidate, MigrationError, MigrationPlan, MigrationResult,
    RecategorizeCandidate, RecategorizePlan, RecategorizeResult, discover_descriptions,
    discover_unmapped_descriptions, execute_migration, execute_recategorize, plan_migration,
    plan_recategorize, sanitize_description,
};

pub const SDK_NAME: &str = "fin-sdk";
pub const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn sdk_banner() -> String {
    format!("{SDK_NAME} v{SDK_VERSION}")
}
