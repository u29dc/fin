use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::extract::rejection::QueryRejection;
use axum::extract::{Path as RoutePath, Query, State};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing::get};
use fin_sdk::config::{LoadedConfig, load_config};
use fin_sdk::rules::{NameMappingConfig, load_rules, resolve_rules_path};
use fin_sdk::{
    AccountBalanceRow, AuditPayeePoint, BalanceSeriesQueryOptions, BalanceSheet, CashflowKpis,
    CashflowTotals, CategoryBreakdownPoint, CategoryMedianPoint, ConfigShowData,
    ConfigValidationResult, ContributionPoint, DailyBalancePoint, DescriptionSummary, EnvelopeMeta,
    ErrorEnvelope, ErrorPayload, ExpenseHierarchyNode, ExpenseHierarchyNodeKind, FinError,
    FinSdkError, FlowGraph, FlowNodeKind, FlowQueryOptions, GlobalFlag, GroupAllocationSnapshot,
    HealthCheckOptions, HealthPoint, HealthReport, HierarchyQueryOptions, JournalEntryRow,
    LedgerQueryOptions, ReserveBreakdownPoint, RollupMode, RuntimeContext, RuntimeContextOptions,
    RunwayPoint, RunwayProjectionOptions, RunwayProjectionReport, SDK_VERSION, SortDirection,
    SuccessEnvelope, SummaryReport, ToolMeta, TransactionCursor, TransactionDetail,
    TransactionListRow, TransactionPageQuery, TransactionSortField, ValidationError,
    account_daily_balance_series, audit_payees, build_config_show, cumulative_contribution_series,
    discover_descriptions, discover_unmapped_descriptions, get_balance_sheet, global_flags,
    group_category_breakdown, group_category_monthly_median, group_daily_balance_series,
    group_expense_hierarchy, group_flow_graph, ledger_entry_count, load_transaction_detail,
    merged_accounts_daily_balance_series, project_consolidated_runway, project_group_runway,
    query_transactions_page, report_cashflow, report_cashflow_kpis,
    report_group_allocation_for_month, report_health, report_reserves, report_runway,
    report_summary, run_health_checks, sdk_banner, tool_registry, validate_config, view_accounts,
    view_ledger,
};
use serde::{Deserialize, Serialize};

use crate::server::BoundEndpoint;

#[derive(Debug, Clone)]
pub struct ApiState {
    pub endpoint: BoundEndpoint,
    pub config_path_override: Option<PathBuf>,
    pub db_path_override: Option<PathBuf>,
}

impl ApiState {
    #[must_use]
    pub fn new(
        endpoint: BoundEndpoint,
        config_path_override: Option<PathBuf>,
        db_path_override: Option<PathBuf>,
    ) -> Self {
        Self {
            endpoint,
            config_path_override,
            db_path_override,
        }
    }
}

type ApiResult<T> = Result<Json<SuccessEnvelope<T>>, ApiError>;

#[derive(Debug, Clone, Default)]
struct MetaExtras {
    count: Option<usize>,
    total: Option<usize>,
    has_more: Option<bool>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    payload: ErrorPayload,
    tool: &'static str,
    started: Instant,
}

impl ApiError {
    fn new(
        status: StatusCode,
        tool: &'static str,
        code: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
        started: Instant,
    ) -> Self {
        Self {
            status,
            payload: ErrorPayload::new(code, message, hint),
            tool,
            started,
        }
    }

    fn not_found(
        tool: &'static str,
        message: impl Into<String>,
        hint: impl Into<String>,
        started: Instant,
    ) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            tool,
            "NOT_FOUND",
            message,
            hint,
            started,
        )
    }

    fn bad_request(
        tool: &'static str,
        code: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
        started: Instant,
    ) -> Self {
        Self::new(StatusCode::BAD_REQUEST, tool, code, message, hint, started)
    }

    fn blocked(
        tool: &'static str,
        code: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
        started: Instant,
    ) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            tool,
            code,
            message,
            hint,
            started,
        )
    }

    fn internal(
        tool: &'static str,
        code: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
        started: Instant,
    ) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            tool,
            code,
            message,
            hint,
            started,
        )
    }

    fn from_fin_error(tool: &'static str, error: FinError, started: Instant) -> Self {
        match error {
            FinError::ConfigNotFound { path } => Self::blocked(
                tool,
                "NO_CONFIG",
                format!("Config file not found: {}", path.display()),
                "Copy fin.config.template.toml into your FIN_HOME data directory.",
                started,
            ),
            FinError::ConfigInvalid { path, message } => Self::blocked(
                tool,
                "INVALID_CONFIG",
                format!("Invalid config at {}: {message}", path.display()),
                "Validate fin.config.toml and retry.",
                started,
            ),
            FinError::RulesNotFound { path } => Self::blocked(
                tool,
                "NO_RULES",
                format!("Rules file not found: {}", path.display()),
                "Create fin.rules.json or run `fin rules migrate-ts`.",
                started,
            ),
            FinError::RulesInvalid { path, message } => Self::blocked(
                tool,
                "INVALID_RULES",
                format!("Invalid rules file at {}: {message}", path.display()),
                "Fix the rules file syntax and required fields.",
                started,
            ),
            FinError::InvalidInput {
                code: "NOT_FOUND",
                message,
            } => Self::not_found(
                tool,
                message,
                "Review request arguments and retry.",
                started,
            ),
            FinError::InvalidInput { code, message } => Self::bad_request(
                tool,
                code,
                message,
                "Review request arguments and retry.",
                started,
            ),
            FinError::Database { message } => Self::internal(
                tool,
                "DB_ERROR",
                format!("Database error: {message}"),
                "Run GET /v1/health and verify the daemon runtime paths.",
                started,
            ),
            FinError::Migration { message } => Self::internal(
                tool,
                "MIGRATION_ERROR",
                format!("Migration error: {message}"),
                "Open the runtime read-write and retry the migration outside fin-api.",
                started,
            ),
            FinError::Io { message } => Self::internal(
                tool,
                "IO_ERROR",
                format!("I/O error: {message}"),
                "Review file permissions and retry.",
                started,
            ),
            FinError::Parse { context, message } => Self::bad_request(
                tool,
                "PARSE_ERROR",
                format!("Parse error ({context}): {message}"),
                "Review request values and retry.",
                started,
            ),
        }
    }

    fn from_sdk_error(tool: &'static str, error: FinSdkError, started: Instant) -> Self {
        match error {
            FinSdkError::ConfigNotFound { path } => Self::blocked(
                tool,
                "NO_CONFIG",
                format!("Config file not found: {path}"),
                "Copy fin.config.template.toml into your FIN_HOME data directory.",
                started,
            ),
            FinSdkError::ConfigRead { path, message } => Self::blocked(
                tool,
                "INVALID_CONFIG",
                format!("Failed to read config file {path}: {message}"),
                "Review file permissions and retry.",
                started,
            ),
            FinSdkError::ConfigParse { path, message } => Self::blocked(
                tool,
                "INVALID_CONFIG",
                format!("Failed to parse config file {path}: {message}"),
                "Validate fin.config.toml and retry.",
                started,
            ),
            FinSdkError::Database { message } => Self::internal(
                tool,
                "DB_ERROR",
                format!("Database error: {message}"),
                "Run GET /v1/health and verify the daemon runtime paths.",
                started,
            ),
            FinSdkError::Runtime { message } => Self::internal(
                tool,
                "RUNTIME_ERROR",
                format!("{tool} failed: {message}"),
                "Review the daemon runtime and retry.",
                started,
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorEnvelope {
            ok: false,
            error: self.payload,
            meta: meta(self.tool, self.started, MetaExtras::default()),
        });
        (self.status, body).into_response()
    }
}

#[derive(Debug, Serialize)]
struct ProbeResponse {
    ok: bool,
    transport: &'static str,
    endpoint: String,
}

#[derive(Debug, Serialize)]
struct VersionPayload {
    tool: &'static str,
    sdk: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolsPayload {
    version: &'static str,
    tools: Vec<ToolMeta>,
    global_flags: Vec<GlobalFlag>,
}

#[derive(Debug, Serialize)]
struct ToolDetailPayload {
    tool: ToolMeta,
}

#[derive(Debug, Deserialize, Default)]
struct RulesPathQuery {
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SanitizeDiscoverQuery {
    #[serde(default)]
    unmapped: bool,
    #[serde(default = "default_discover_min")]
    min: usize,
    #[serde(default = "default_discover_limit")]
    limit: usize,
    account: Option<String>,
}

impl Default for SanitizeDiscoverQuery {
    fn default() -> Self {
        Self {
            unmapped: false,
            min: default_discover_min(),
            limit: default_discover_limit(),
            account: None,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct ViewAccountsQuery {
    group: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewTransactionsQuery {
    account: Option<String>,
    group: Option<String>,
    from: Option<String>,
    to: Option<String>,
    search: Option<String>,
    #[serde(default = "default_page_limit")]
    limit: usize,
    #[serde(default = "default_sort_field")]
    sort_field: TransactionSortField,
    #[serde(default = "default_sort_direction")]
    sort_direction: SortDirection,
    after: Option<String>,
}

impl Default for ViewTransactionsQuery {
    fn default() -> Self {
        Self {
            account: None,
            group: None,
            from: None,
            to: None,
            search: None,
            limit: default_page_limit(),
            sort_field: default_sort_field(),
            sort_direction: default_sort_direction(),
            after: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewLedgerQuery {
    account: Option<String>,
    from: Option<String>,
    to: Option<String>,
    #[serde(default = "default_page_limit")]
    limit: usize,
}

impl Default for ViewLedgerQuery {
    fn default() -> Self {
        Self {
            account: None,
            from: None,
            to: None,
            limit: default_page_limit(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ViewBalanceQuery {
    as_of: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportCashflowQuery {
    group: String,
    #[serde(default = "default_report_months")]
    months: usize,
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportHealthQuery {
    group: String,
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportRunwayQuery {
    group: Option<String>,
    #[serde(default)]
    consolidated: bool,
    include: Option<String>,
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportReservesQuery {
    group: String,
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportCategoriesQuery {
    group: String,
    #[serde(default = "default_categories_mode")]
    mode: String,
    #[serde(default = "default_categories_months")]
    months: usize,
    #[serde(default = "default_categories_limit")]
    limit: usize,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportAuditQuery {
    account: String,
    #[serde(default = "default_audit_months")]
    months: usize,
    #[serde(default = "default_audit_limit")]
    limit: usize,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportSummaryQuery {
    #[serde(default = "default_report_months")]
    months: usize,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardKpisQuery {
    group: String,
    #[serde(default = "default_dashboard_kpi_months")]
    months: usize,
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DashboardAllocationQuery {
    group: String,
    month: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardHierarchyQuery {
    group: String,
    #[serde(default = "default_dashboard_insight_months")]
    months: usize,
    #[serde(default = "default_rollup_mode")]
    mode: RollupMode,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardFlowQuery {
    group: String,
    #[serde(default = "default_dashboard_insight_months")]
    months: usize,
    #[serde(default = "default_rollup_mode")]
    mode: RollupMode,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardBalanceQuery {
    group: Option<String>,
    account: Option<String>,
    from: Option<String>,
    to: Option<String>,
    #[serde(default = "default_series_limit")]
    limit: usize,
    downsample_min_step_days: Option<u32>,
}

impl Default for DashboardBalanceQuery {
    fn default() -> Self {
        Self {
            group: None,
            account: None,
            from: None,
            to: None,
            limit: default_series_limit(),
            downsample_min_step_days: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardContributionQuery {
    account: String,
    from: Option<String>,
    to: Option<String>,
    #[serde(default = "default_series_limit")]
    limit: usize,
    downsample_min_step_days: Option<u32>,
}

impl Default for DashboardContributionQuery {
    fn default() -> Self {
        Self {
            account: String::new(),
            from: None,
            to: None,
            limit: default_series_limit(),
            downsample_min_step_days: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DashboardProjectionQuery {
    group: Option<String>,
    #[serde(default)]
    consolidated: bool,
    include: Option<String>,
    #[serde(default = "default_projection_months")]
    months: usize,
    #[serde(default = "default_projection_minimum_burn_ratio")]
    minimum_burn_ratio: f64,
    as_of: Option<String>,
    trailing_outflow_window_months: Option<usize>,
}

impl Default for DashboardProjectionQuery {
    fn default() -> Self {
        Self {
            group: None,
            consolidated: false,
            include: None,
            months: default_projection_months(),
            minimum_burn_ratio: default_projection_minimum_burn_ratio(),
            as_of: None,
            trailing_outflow_window_months: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RulesShowPayload {
    rules_path: String,
    external_loaded: bool,
    rule_count: usize,
    warn_on_unmapped: bool,
    fallback_to_raw: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RulesValidationPayload {
    valid: bool,
    errors: Vec<ValidationError>,
    rules_path: String,
    external_loaded: bool,
    rule_count: usize,
}

#[derive(Debug, Serialize)]
struct SanitizeDiscoverPayload {
    descriptions: Vec<DescriptionSummary>,
    count: usize,
}

#[derive(Debug, Serialize)]
struct ViewAccountsPayload {
    accounts: Vec<AccountBalanceRow>,
    total: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TransactionPagePayload {
    items: Vec<TransactionListRow>,
    count: usize,
    total_count: usize,
    has_more: bool,
    next_cursor: Option<TransactionCursor>,
    next_cursor_token: Option<String>,
}

#[derive(Debug, Serialize)]
struct LedgerPayload {
    entries: Vec<JournalEntryRow>,
    count: usize,
    total: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BalancePayload {
    assets: i64,
    liabilities: i64,
    equity: i64,
    income: i64,
    expenses: i64,
    net_worth: i64,
    net_income: i64,
}

impl From<BalanceSheet> for BalancePayload {
    fn from(value: BalanceSheet) -> Self {
        Self {
            assets: value.assets,
            liabilities: value.liabilities,
            equity: value.equity,
            income: value.income,
            expenses: value.expenses,
            net_worth: value.net_worth,
            net_income: value.net_income,
        }
    }
}

#[derive(Debug, Serialize)]
struct CashflowPayload {
    series: Vec<fin_sdk::MonthlyCashflowPoint>,
    totals: CashflowTotals,
}

#[derive(Debug, Serialize)]
struct HealthSeriesPayload {
    series: Vec<HealthPoint>,
    latest: Option<HealthPoint>,
}

#[derive(Debug, Serialize)]
struct RunwayPayload {
    series: Vec<RunwayPoint>,
    latest: Option<RunwayPoint>,
    groups: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ReservesPayload {
    series: Vec<ReserveBreakdownPoint>,
    latest: Option<ReserveBreakdownPoint>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BreakdownCategoriesPayload {
    mode: &'static str,
    categories: Vec<CategoryBreakdownPoint>,
    total: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MedianCategoriesPayload {
    mode: &'static str,
    categories: Vec<CategoryMedianPoint>,
    estimated_monthly: i64,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum CategoriesPayload {
    Breakdown(BreakdownCategoriesPayload),
    Median(MedianCategoriesPayload),
}

#[derive(Debug, Serialize)]
struct AuditPayload {
    payees: Vec<AuditPayeePoint>,
    total: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardKpisPayload {
    group_id: String,
    group_label: String,
    months: usize,
    kpis: CashflowKpis,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardAllocationPayload {
    reporting_month: String,
    snapshot: GroupAllocationSnapshot,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardHierarchyPayload {
    group_id: String,
    months: usize,
    mode: RollupMode,
    total_minor: i64,
    nodes: Vec<ExpenseHierarchyNode>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardFlowPayload {
    group_id: String,
    months: usize,
    mode: RollupMode,
    graph: FlowGraph,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum DashboardBalanceScopeKind {
    AllAssets,
    Group,
    Account,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardBalancePayload {
    scope_kind: DashboardBalanceScopeKind,
    scope_id: String,
    scope_label: String,
    series: Vec<DailyBalancePoint>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardContributionPayload {
    account_id: String,
    account_label: String,
    series: Vec<ContributionPoint>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardProjectionPayload {
    groups: Vec<String>,
    report: RunwayProjectionReport,
}

pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/__probe", get(probe_handler))
        .route("/v1/version", get(version_handler))
        .route("/v1/tools", get(tools_handler))
        .route("/v1/tools/{name}", get(tool_detail_handler))
        .route("/v1/health", get(health_handler))
        .route("/v1/config/show", get(config_show_handler))
        .route("/v1/config/validate", get(config_validate_handler))
        .route("/v1/rules/show", get(rules_show_handler))
        .route("/v1/rules/validate", get(rules_validate_handler))
        .route("/v1/sanitize/discover", get(sanitize_discover_handler))
        .route("/v1/view/accounts", get(view_accounts_handler))
        .route("/v1/view/transactions", get(view_transactions_handler))
        .route(
            "/v1/view/transactions/{posting_id}",
            get(view_transaction_detail_handler),
        )
        .route("/v1/view/ledger", get(view_ledger_handler))
        .route("/v1/view/balance", get(view_balance_handler))
        .route("/v1/report/cashflow", get(report_cashflow_handler))
        .route("/v1/report/health", get(report_health_handler))
        .route("/v1/report/runway", get(report_runway_handler))
        .route("/v1/report/reserves", get(report_reserves_handler))
        .route("/v1/report/categories", get(report_categories_handler))
        .route("/v1/report/audit", get(report_audit_handler))
        .route("/v1/report/summary", get(report_summary_handler))
        .route("/v1/dashboard/kpis", get(dashboard_kpis_handler))
        .route(
            "/v1/dashboard/allocation",
            get(dashboard_allocation_handler),
        )
        .route("/v1/dashboard/hierarchy", get(dashboard_hierarchy_handler))
        .route("/v1/dashboard/flow", get(dashboard_flow_handler))
        .route("/v1/dashboard/balances", get(dashboard_balances_handler))
        .route(
            "/v1/dashboard/contributions",
            get(dashboard_contributions_handler),
        )
        .route(
            "/v1/dashboard/projection",
            get(dashboard_projection_handler),
        )
        .fallback(fallback_handler)
        .with_state(state)
}

async fn probe_handler(State(state): State<ApiState>) -> Json<ProbeResponse> {
    Json(ProbeResponse {
        ok: true,
        transport: state.endpoint.transport_name(),
        endpoint: state.endpoint.endpoint_label(),
    })
}

async fn version_handler() -> Json<SuccessEnvelope<VersionPayload>> {
    let started = Instant::now();
    success(
        "version",
        VersionPayload {
            tool: "version",
            sdk: sdk_banner(),
        },
        started,
        MetaExtras::default(),
    )
}

async fn tools_handler() -> Json<SuccessEnvelope<ToolsPayload>> {
    let started = Instant::now();
    let tools = tool_registry();
    let count = tools.len();
    success(
        "tools",
        ToolsPayload {
            version: SDK_VERSION,
            tools,
            global_flags: global_flags(),
        },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    )
}

async fn tool_detail_handler(
    RoutePath(name): RoutePath<String>,
) -> Result<Json<SuccessEnvelope<ToolDetailPayload>>, ApiError> {
    let started = Instant::now();
    let Some(tool) = tool_registry()
        .into_iter()
        .find(|candidate| candidate.name == name)
    else {
        return Err(ApiError::not_found(
            "tools",
            format!("tool \"{name}\" not found"),
            "Call GET /v1/tools to list all available tools.",
            started,
        ));
    };

    Ok(success(
        "tools",
        ToolDetailPayload { tool },
        started,
        MetaExtras::default(),
    ))
}

async fn health_handler(State(state): State<ApiState>) -> Json<SuccessEnvelope<HealthReport>> {
    let started = Instant::now();
    let report = run_health_checks(HealthCheckOptions {
        config_path: state.config_path_override.clone(),
        db_path: state.db_path_override.clone(),
        ..HealthCheckOptions::default()
    });
    success("health", report, started, MetaExtras::default())
}

async fn config_show_handler(State(state): State<ApiState>) -> ApiResult<ConfigShowData> {
    let started = Instant::now();
    let data = build_config_show(state.config_path_override.as_deref())
        .map_err(|error| ApiError::from_sdk_error("config.show", error, started))?;
    Ok(success("config.show", data, started, MetaExtras::default()))
}

async fn config_validate_handler(
    State(state): State<ApiState>,
) -> ApiResult<ConfigValidationResult> {
    let started = Instant::now();
    let data = validate_config(state.config_path_override.as_deref())
        .map_err(|error| ApiError::from_sdk_error("config.validate", error, started))?;
    Ok(success(
        "config.validate",
        data,
        started,
        MetaExtras::default(),
    ))
}

async fn rules_show_handler(
    State(state): State<ApiState>,
    query: Result<Query<RulesPathQuery>, QueryRejection>,
) -> ApiResult<RulesShowPayload> {
    let started = Instant::now();
    let query = parse_query(query, "rules.show", started)?;
    let loaded_config = load_rules_config(&state, "rules.show", started)?;
    let explicit_path = query.path.as_deref().map(Path::new);
    let loaded = load_rules(
        explicit_path,
        loaded_config.as_ref(),
        None::<NameMappingConfig>,
    )
    .map_err(|error| ApiError::from_fin_error("rules.show", error, started))?;

    let payload = RulesShowPayload {
        rules_path: loaded.resolved_path.display().to_string(),
        external_loaded: loaded.external_loaded,
        rule_count: loaded.config.rules.len(),
        warn_on_unmapped: loaded.config.warn_on_unmapped,
        fallback_to_raw: loaded.config.fallback_to_raw,
    };

    Ok(success(
        "rules.show",
        payload,
        started,
        MetaExtras::default(),
    ))
}

async fn rules_validate_handler(
    State(state): State<ApiState>,
    query: Result<Query<RulesPathQuery>, QueryRejection>,
) -> ApiResult<RulesValidationPayload> {
    let started = Instant::now();
    let query = parse_query(query, "rules.validate", started)?;
    let loaded_config = load_rules_config(&state, "rules.validate", started)?;
    let explicit_path = query.path.as_deref().map(Path::new);
    let resolved_path = resolve_rules_path(explicit_path, loaded_config.as_ref());

    let payload = match load_rules(
        explicit_path,
        loaded_config.as_ref(),
        None::<NameMappingConfig>,
    ) {
        Ok(loaded) => RulesValidationPayload {
            valid: true,
            errors: Vec::new(),
            rules_path: resolved_path.display().to_string(),
            external_loaded: loaded.external_loaded,
            rule_count: loaded.config.rules.len(),
        },
        Err(error) => RulesValidationPayload {
            valid: false,
            errors: vec![ValidationError {
                path: "$".to_owned(),
                message: error.to_string(),
            }],
            rules_path: resolved_path.display().to_string(),
            external_loaded: false,
            rule_count: 0,
        },
    };

    Ok(success(
        "rules.validate",
        payload,
        started,
        MetaExtras::default(),
    ))
}

async fn sanitize_discover_handler(
    State(state): State<ApiState>,
    query: Result<Query<SanitizeDiscoverQuery>, QueryRejection>,
) -> ApiResult<SanitizeDiscoverPayload> {
    let started = Instant::now();
    let query = parse_query(query, "sanitize.discover", started)?;
    let runtime = open_read_runtime(&state, "sanitize.discover", started)?;
    let rules = runtime
        .load_rules(None, None)
        .map_err(|error| ApiError::from_fin_error("sanitize.discover", error, started))?;

    let descriptions = if query.unmapped {
        discover_unmapped_descriptions(
            runtime.connection(),
            &rules.config,
            query.min,
            query.account.as_deref(),
            query.limit,
        )
    } else {
        discover_descriptions(
            runtime.connection(),
            query.min,
            query.account.as_deref(),
            query.limit,
        )
    }
    .map_err(|error| ApiError::from_fin_error("sanitize.discover", error, started))?;

    let count = descriptions.len();
    Ok(success(
        "sanitize.discover",
        SanitizeDiscoverPayload {
            descriptions,
            count,
        },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn view_accounts_handler(
    State(state): State<ApiState>,
    query: Result<Query<ViewAccountsQuery>, QueryRejection>,
) -> ApiResult<ViewAccountsPayload> {
    let started = Instant::now();
    let query = parse_query(query, "view.accounts", started)?;
    let runtime = open_read_runtime(&state, "view.accounts", started)?;
    let accounts = view_accounts(
        runtime.connection(),
        runtime.config(),
        query.group.as_deref(),
    )
    .map_err(|error| ApiError::from_fin_error("view.accounts", error, started))?;
    let total = accounts
        .iter()
        .map(|account| account.balance_minor.unwrap_or(0))
        .sum::<i64>();
    let count = accounts.len();

    Ok(success(
        "view.accounts",
        ViewAccountsPayload { accounts, total },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn view_transactions_handler(
    State(state): State<ApiState>,
    query: Result<Query<ViewTransactionsQuery>, QueryRejection>,
) -> ApiResult<TransactionPagePayload> {
    let started = Instant::now();
    let query = parse_query(query, "view.transactions", started)?;
    if query.account.is_some() && query.group.is_some() {
        return Err(ApiError::bad_request(
            "view.transactions",
            "INVALID_INPUT",
            "account and group filters cannot be combined",
            "Pass either account or group for transaction paging, not both.",
            started,
        ));
    }

    let after = match query.after.as_deref() {
        Some(token) => Some(parse_cursor_token(token, "view.transactions", started)?),
        None => None,
    };
    let runtime = open_read_runtime(&state, "view.transactions", started)?;
    let request = TransactionPageQuery {
        group_id: query.group.clone(),
        chart_account_ids: query
            .account
            .as_ref()
            .map(|account_id| vec![account_id.clone()]),
        from: query.from.clone(),
        to: query.to.clone(),
        search: query.search.clone(),
        limit: query.limit,
        sort_field: query.sort_field,
        sort_direction: query.sort_direction,
        after,
    };
    let page = query_transactions_page(runtime.connection(), runtime.config(), &request)
        .map_err(|error| ApiError::from_fin_error("view.transactions", error, started))?;
    let next_cursor_token = page
        .next_cursor
        .as_ref()
        .map(serialize_cursor_token)
        .transpose()
        .map_err(|error| {
            ApiError::internal(
                "view.transactions",
                "RUNTIME_ERROR",
                error,
                "Retry the request.",
                started,
            )
        })?;

    let total_count = page.total_count;
    let has_more = page.has_more;
    let count = page.items.len();
    Ok(success(
        "view.transactions",
        TransactionPagePayload {
            items: page.items,
            count,
            total_count,
            has_more,
            next_cursor: page.next_cursor,
            next_cursor_token,
        },
        started,
        MetaExtras {
            count: Some(count),
            total: Some(total_count),
            has_more: Some(has_more),
        },
    ))
}

async fn view_transaction_detail_handler(
    State(state): State<ApiState>,
    RoutePath(posting_id): RoutePath<String>,
) -> ApiResult<TransactionDetail> {
    let started = Instant::now();
    let runtime = open_read_runtime(&state, "view.transactions", started)?;
    let Some(detail) = load_transaction_detail(runtime.connection(), &posting_id)
        .map_err(|error| ApiError::from_fin_error("view.transactions", error, started))?
    else {
        return Err(ApiError::not_found(
            "view.transactions",
            format!("transaction posting \"{posting_id}\" not found"),
            "Request an existing posting_id from GET /v1/view/transactions.",
            started,
        ));
    };

    Ok(success(
        "view.transactions",
        detail,
        started,
        MetaExtras::default(),
    ))
}

async fn view_ledger_handler(
    State(state): State<ApiState>,
    query: Result<Query<ViewLedgerQuery>, QueryRejection>,
) -> ApiResult<LedgerPayload> {
    let started = Instant::now();
    let query = parse_query(query, "view.ledger", started)?;
    let runtime = open_read_runtime(&state, "view.ledger", started)?;
    let entries = view_ledger(
        runtime.connection(),
        &LedgerQueryOptions {
            account_id: query.account.clone(),
            from: query.from.clone(),
            to: query.to.clone(),
            limit: query.limit,
        },
    )
    .map_err(|error| ApiError::from_fin_error("view.ledger", error, started))?;
    let total = ledger_entry_count(runtime.connection(), query.account.as_deref())
        .map_err(|error| ApiError::from_fin_error("view.ledger", error, started))?;
    let total = usize::try_from(total).unwrap_or_default();
    let count = entries.len();

    Ok(success(
        "view.ledger",
        LedgerPayload {
            entries,
            count,
            total,
        },
        started,
        MetaExtras {
            count: Some(count),
            total: Some(total),
            has_more: Some(count < total),
        },
    ))
}

async fn view_balance_handler(
    State(state): State<ApiState>,
    query: Result<Query<ViewBalanceQuery>, QueryRejection>,
) -> ApiResult<BalancePayload> {
    let started = Instant::now();
    let query = parse_query(query, "view.balance", started)?;
    let runtime = open_read_runtime(&state, "view.balance", started)?;
    let sheet = get_balance_sheet(runtime.connection(), query.as_of.as_deref())
        .map_err(|error| ApiError::from_fin_error("view.balance", error, started))?;

    Ok(success(
        "view.balance",
        BalancePayload::from(sheet),
        started,
        MetaExtras::default(),
    ))
}

async fn report_cashflow_handler(
    State(state): State<ApiState>,
    query: Result<Query<ReportCashflowQuery>, QueryRejection>,
) -> ApiResult<CashflowPayload> {
    let started = Instant::now();
    let query = parse_query(query, "report.cashflow", started)?;
    let runtime = open_read_runtime(&state, "report.cashflow", started)?;
    let (series, totals) = report_cashflow(
        runtime.connection(),
        runtime.config(),
        &query.group,
        query.months,
        query.from.as_deref(),
        query.to.as_deref(),
    )
    .map_err(|error| ApiError::from_fin_error("report.cashflow", error, started))?;
    let count = series.len();

    Ok(success(
        "report.cashflow",
        CashflowPayload { series, totals },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn report_health_handler(
    State(state): State<ApiState>,
    query: Result<Query<ReportHealthQuery>, QueryRejection>,
) -> ApiResult<HealthSeriesPayload> {
    let started = Instant::now();
    let query = parse_query(query, "report.health", started)?;
    let runtime = open_read_runtime(&state, "report.health", started)?;
    let series = report_health(
        runtime.connection(),
        runtime.config(),
        &query.group,
        query.from.as_deref(),
        query.to.as_deref(),
    )
    .map_err(|error| ApiError::from_fin_error("report.health", error, started))?;
    let latest = series.last().cloned();
    let count = series.len();

    Ok(success(
        "report.health",
        HealthSeriesPayload { series, latest },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn report_runway_handler(
    State(state): State<ApiState>,
    query: Result<Query<ReportRunwayQuery>, QueryRejection>,
) -> ApiResult<RunwayPayload> {
    let started = Instant::now();
    let query = parse_query(query, "report.runway", started)?;
    let runtime = open_read_runtime(&state, "report.runway", started)?;

    let payload = if query.consolidated {
        let groups = selected_runway_groups(runtime.config(), query.include.as_deref());
        let series = consolidate_runway_series(
            &runtime,
            &groups,
            query.from.as_deref(),
            query.to.as_deref(),
            started,
        )?;
        let latest = series.last().cloned();
        RunwayPayload {
            series,
            latest,
            groups,
        }
    } else {
        let group = query.group.ok_or_else(|| {
            ApiError::bad_request(
                "report.runway",
                "INVALID_INPUT",
                "Missing group for runway report",
                "Pass group or set consolidated=true for a combined runway series.",
                started,
            )
        })?;
        let series = report_runway(
            runtime.connection(),
            runtime.config(),
            &group,
            query.from.as_deref(),
            query.to.as_deref(),
        )
        .map_err(|error| ApiError::from_fin_error("report.runway", error, started))?;
        let latest = series.last().cloned();
        RunwayPayload {
            series,
            latest,
            groups: vec![group],
        }
    };
    let count = payload.series.len();

    Ok(success(
        "report.runway",
        payload,
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn report_reserves_handler(
    State(state): State<ApiState>,
    query: Result<Query<ReportReservesQuery>, QueryRejection>,
) -> ApiResult<ReservesPayload> {
    let started = Instant::now();
    let query = parse_query(query, "report.reserves", started)?;
    let runtime = open_read_runtime(&state, "report.reserves", started)?;
    let series = report_reserves(
        runtime.connection(),
        runtime.config(),
        &query.group,
        query.from.as_deref(),
        query.to.as_deref(),
    )
    .map_err(|error| ApiError::from_fin_error("report.reserves", error, started))?;
    let latest = series.last().cloned();
    let count = series.len();

    Ok(success(
        "report.reserves",
        ReservesPayload { series, latest },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn report_categories_handler(
    State(state): State<ApiState>,
    query: Result<Query<ReportCategoriesQuery>, QueryRejection>,
) -> ApiResult<CategoriesPayload> {
    let started = Instant::now();
    let query = parse_query(query, "report.categories", started)?;
    let runtime = open_read_runtime(&state, "report.categories", started)?;

    let (payload, count) = match query.mode.as_str() {
        "breakdown" => {
            let categories = group_category_breakdown(
                runtime.connection(),
                runtime.config(),
                &query.group,
                query.months,
                query.limit,
                query.to.as_deref(),
            )
            .map_err(|error| ApiError::from_fin_error("report.categories", error, started))?;
            let total = categories
                .iter()
                .map(|point| point.total_minor)
                .sum::<i64>();
            let count = categories.len();
            (
                CategoriesPayload::Breakdown(BreakdownCategoriesPayload {
                    mode: "breakdown",
                    categories,
                    total,
                }),
                count,
            )
        }
        "median" => {
            let categories = group_category_monthly_median(
                runtime.connection(),
                runtime.config(),
                &query.group,
                query.months,
                query.limit,
                query.to.as_deref(),
            )
            .map_err(|error| ApiError::from_fin_error("report.categories", error, started))?;
            let estimated_monthly = categories
                .iter()
                .map(|point| point.monthly_median_minor)
                .sum::<i64>();
            let count = categories.len();
            (
                CategoriesPayload::Median(MedianCategoriesPayload {
                    mode: "median",
                    categories,
                    estimated_monthly,
                }),
                count,
            )
        }
        other => {
            return Err(ApiError::bad_request(
                "report.categories",
                "INVALID_INPUT",
                format!("Unsupported categories mode: {other}"),
                "Use mode=breakdown or mode=median.",
                started,
            ));
        }
    };

    Ok(success(
        "report.categories",
        payload,
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn report_audit_handler(
    State(state): State<ApiState>,
    query: Result<Query<ReportAuditQuery>, QueryRejection>,
) -> ApiResult<AuditPayload> {
    let started = Instant::now();
    let query = parse_query(query, "report.audit", started)?;
    let runtime = open_read_runtime(&state, "report.audit", started)?;
    let payees = audit_payees(
        runtime.connection(),
        &query.account,
        query.months,
        query.limit,
        query.to.as_deref(),
    )
    .map_err(|error| ApiError::from_fin_error("report.audit", error, started))?;
    let total = payees.iter().map(|point| point.total_minor).sum::<i64>();
    let count = payees.len();

    Ok(success(
        "report.audit",
        AuditPayload { payees, total },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn report_summary_handler(
    State(state): State<ApiState>,
    query: Result<Query<ReportSummaryQuery>, QueryRejection>,
) -> ApiResult<SummaryReport> {
    let started = Instant::now();
    let query = parse_query(query, "report.summary", started)?;
    let runtime = open_read_runtime(&state, "report.summary", started)?;
    let report = report_summary(
        runtime.connection(),
        runtime.config(),
        query.months,
        query.to.as_deref(),
    )
    .map_err(|error| ApiError::from_fin_error("report.summary", error, started))?;
    let count = report.groups.len();

    Ok(success(
        "report.summary",
        report,
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn dashboard_kpis_handler(
    State(state): State<ApiState>,
    query: Result<Query<DashboardKpisQuery>, QueryRejection>,
) -> ApiResult<DashboardKpisPayload> {
    let started = Instant::now();
    let query = parse_query(query, "dashboard.kpis", started)?;
    let runtime = open_read_runtime(&state, "dashboard.kpis", started)?;
    ensure_group_exists(runtime.config(), &query.group, "dashboard.kpis", started)?;

    let kpis = report_cashflow_kpis(
        runtime.connection(),
        runtime.config(),
        &query.group,
        query.months,
        query.from.as_deref(),
        query.to.as_deref(),
    )
    .map_err(|error| ApiError::from_fin_error("dashboard.kpis", error, started))?;
    let group_label = runtime.config().resolve_group_metadata(&query.group).label;

    Ok(success(
        "dashboard.kpis",
        DashboardKpisPayload {
            group_id: query.group,
            group_label,
            months: query.months,
            kpis,
        },
        started,
        MetaExtras::default(),
    ))
}

async fn dashboard_allocation_handler(
    State(state): State<ApiState>,
    query: Result<Query<DashboardAllocationQuery>, QueryRejection>,
) -> ApiResult<DashboardAllocationPayload> {
    let started = Instant::now();
    let query = parse_query(query, "dashboard.allocation", started)?;
    let runtime = open_read_runtime(&state, "dashboard.allocation", started)?;
    ensure_group_exists(
        runtime.config(),
        &query.group,
        "dashboard.allocation",
        started,
    )?;

    let requested_month = query.month.clone();
    let reporting_month = match requested_month {
        Some(month) => {
            validate_year_month(&month, "dashboard.allocation", started)?;
            month
        }
        None => fin_sdk::current_reporting_month(runtime.connection())
            .map_err(|error| ApiError::from_fin_error("dashboard.allocation", error, started))?,
    };
    let snapshot = report_group_allocation_for_month(
        runtime.connection(),
        runtime.config(),
        &query.group,
        &reporting_month,
    )
    .map_err(|error| ApiError::from_fin_error("dashboard.allocation", error, started))?;

    Ok(success(
        "dashboard.allocation",
        DashboardAllocationPayload {
            reporting_month,
            snapshot,
        },
        started,
        MetaExtras {
            count: Some(1),
            ..MetaExtras::default()
        },
    ))
}

async fn dashboard_hierarchy_handler(
    State(state): State<ApiState>,
    query: Result<Query<DashboardHierarchyQuery>, QueryRejection>,
) -> ApiResult<DashboardHierarchyPayload> {
    let started = Instant::now();
    let query = parse_query(query, "dashboard.hierarchy", started)?;
    let runtime = open_read_runtime(&state, "dashboard.hierarchy", started)?;
    ensure_group_exists(
        runtime.config(),
        &query.group,
        "dashboard.hierarchy",
        started,
    )?;

    let raw_nodes = group_expense_hierarchy(
        runtime.connection(),
        runtime.config(),
        &query.group,
        &HierarchyQueryOptions {
            months: query.months,
            mode: query.mode,
            to: query.to.clone(),
        },
    )
    .map_err(|error| ApiError::from_fin_error("dashboard.hierarchy", error, started))?;
    let nodes = filter_expense_hierarchy_nodes(&raw_nodes);
    let count = hierarchy_node_count(&nodes);
    let total_minor = nodes.iter().map(|node| node.total_minor).sum::<i64>();

    Ok(success(
        "dashboard.hierarchy",
        DashboardHierarchyPayload {
            group_id: query.group,
            months: query.months,
            mode: query.mode,
            total_minor,
            nodes,
        },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn dashboard_flow_handler(
    State(state): State<ApiState>,
    query: Result<Query<DashboardFlowQuery>, QueryRejection>,
) -> ApiResult<DashboardFlowPayload> {
    let started = Instant::now();
    let query = parse_query(query, "dashboard.flow", started)?;
    let runtime = open_read_runtime(&state, "dashboard.flow", started)?;
    ensure_group_exists(runtime.config(), &query.group, "dashboard.flow", started)?;

    let raw_graph = group_flow_graph(
        runtime.connection(),
        runtime.config(),
        &query.group,
        &FlowQueryOptions {
            months: query.months,
            mode: query.mode,
            to: query.to.clone(),
        },
    )
    .map_err(|error| ApiError::from_fin_error("dashboard.flow", error, started))?;
    let graph = filter_dashboard_flow_graph(raw_graph);
    let count = graph.edges.len();

    Ok(success(
        "dashboard.flow",
        DashboardFlowPayload {
            group_id: query.group,
            months: query.months,
            mode: query.mode,
            graph,
        },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn dashboard_balances_handler(
    State(state): State<ApiState>,
    query: Result<Query<DashboardBalanceQuery>, QueryRejection>,
) -> ApiResult<DashboardBalancePayload> {
    let started = Instant::now();
    let query = parse_query(query, "dashboard.balances", started)?;
    if query.group.is_some() && query.account.is_some() {
        return Err(ApiError::bad_request(
            "dashboard.balances",
            "INVALID_INPUT",
            "group and account filters cannot be combined",
            "Pass either group or account for dashboard balances, not both.",
            started,
        ));
    }

    let runtime = open_read_runtime(&state, "dashboard.balances", started)?;
    let options = build_balance_series_options(
        query.from.clone(),
        query.to.clone(),
        query.limit,
        query.downsample_min_step_days,
    );

    let payload = if let Some(account_id) = query.account {
        let account =
            ensure_account_exists(runtime.config(), &account_id, "dashboard.balances", started)?;
        let series = account_daily_balance_series(runtime.connection(), &account_id, &options)
            .map_err(|error| ApiError::from_fin_error("dashboard.balances", error, started))?;
        DashboardBalancePayload {
            scope_kind: DashboardBalanceScopeKind::Account,
            scope_id: account_id,
            scope_label: account.label.clone().unwrap_or_else(|| account.id.clone()),
            series,
        }
    } else if let Some(group_id) = query.group {
        ensure_group_exists(runtime.config(), &group_id, "dashboard.balances", started)?;
        let group = runtime.config().resolve_group_metadata(&group_id);
        let series =
            group_daily_balance_series(runtime.connection(), runtime.config(), &group_id, &options)
                .map_err(|error| ApiError::from_fin_error("dashboard.balances", error, started))?;
        DashboardBalancePayload {
            scope_kind: DashboardBalanceScopeKind::Group,
            scope_id: group_id,
            scope_label: group.label,
            series,
        }
    } else {
        let account_ids = runtime
            .config()
            .accounts
            .iter()
            .filter(|account| account.account_type == "asset")
            .map(|account| account.id.clone())
            .collect::<Vec<_>>();
        let series =
            merged_accounts_daily_balance_series(runtime.connection(), &account_ids, &options)
                .map_err(|error| ApiError::from_fin_error("dashboard.balances", error, started))?;
        DashboardBalancePayload {
            scope_kind: DashboardBalanceScopeKind::AllAssets,
            scope_id: "all-assets".to_owned(),
            scope_label: "All assets".to_owned(),
            series,
        }
    };
    let count = payload.series.len();

    Ok(success(
        "dashboard.balances",
        payload,
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn dashboard_contributions_handler(
    State(state): State<ApiState>,
    query: Result<Query<DashboardContributionQuery>, QueryRejection>,
) -> ApiResult<DashboardContributionPayload> {
    let started = Instant::now();
    let query = parse_query(query, "dashboard.contributions", started)?;
    let runtime = open_read_runtime(&state, "dashboard.contributions", started)?;
    let account = ensure_account_exists(
        runtime.config(),
        &query.account,
        "dashboard.contributions",
        started,
    )?;
    let series = cumulative_contribution_series(
        runtime.connection(),
        &query.account,
        &build_balance_series_options(
            query.from,
            query.to,
            query.limit,
            query.downsample_min_step_days,
        ),
    )
    .map_err(|error| ApiError::from_fin_error("dashboard.contributions", error, started))?;
    let count = series.len();

    Ok(success(
        "dashboard.contributions",
        DashboardContributionPayload {
            account_id: query.account,
            account_label: account.label.clone().unwrap_or_else(|| account.id.clone()),
            series,
        },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn dashboard_projection_handler(
    State(state): State<ApiState>,
    query: Result<Query<DashboardProjectionQuery>, QueryRejection>,
) -> ApiResult<DashboardProjectionPayload> {
    let started = Instant::now();
    let query = parse_query(query, "dashboard.projection", started)?;
    let runtime = open_read_runtime(&state, "dashboard.projection", started)?;
    let options = RunwayProjectionOptions {
        months: query.months,
        minimum_burn_ratio: query.minimum_burn_ratio,
        as_of: query.as_of.clone(),
        trailing_outflow_window_months: query.trailing_outflow_window_months,
    };

    let (groups, report) = if query.consolidated {
        if query.group.is_some() {
            return Err(ApiError::bad_request(
                "dashboard.projection",
                "INVALID_INPUT",
                "group cannot be combined with consolidated=true",
                "Pass either group for one scope or consolidated=true with optional include.",
                started,
            ));
        }
        let groups = validated_selected_groups(
            runtime.config(),
            query.include.as_deref(),
            "dashboard.projection",
            started,
        )?;
        let report =
            project_consolidated_runway(runtime.connection(), runtime.config(), &groups, &options)
                .map_err(|error| {
                    ApiError::from_fin_error("dashboard.projection", error, started)
                })?;
        (groups, report)
    } else {
        if query.include.is_some() {
            return Err(ApiError::bad_request(
                "dashboard.projection",
                "INVALID_INPUT",
                "include requires consolidated=true",
                "Set consolidated=true when passing include.",
                started,
            ));
        }
        let group_id = query.group.ok_or_else(|| {
            ApiError::bad_request(
                "dashboard.projection",
                "INVALID_INPUT",
                "Missing group for dashboard projection",
                "Pass group or set consolidated=true for a combined projection.",
                started,
            )
        })?;
        ensure_group_exists(runtime.config(), &group_id, "dashboard.projection", started)?;
        let report =
            project_group_runway(runtime.connection(), runtime.config(), &group_id, &options)
                .map_err(|error| {
                    ApiError::from_fin_error("dashboard.projection", error, started)
                })?;
        (vec![group_id], report)
    };
    let count = report.scenarios.len();

    Ok(success(
        "dashboard.projection",
        DashboardProjectionPayload { groups, report },
        started,
        MetaExtras {
            count: Some(count),
            ..MetaExtras::default()
        },
    ))
}

async fn fallback_handler(uri: Uri) -> ApiError {
    ApiError::not_found(
        "api",
        format!("route {} not found", uri.path()),
        "Call GET /v1/tools to inspect available capabilities.",
        Instant::now(),
    )
}

fn parse_query<T>(
    query: Result<Query<T>, QueryRejection>,
    tool: &'static str,
    started: Instant,
) -> Result<T, ApiError> {
    query.map(|Query(value)| value).map_err(|error| {
        ApiError::bad_request(
            tool,
            "INVALID_INPUT",
            format!("Invalid query parameters: {error}"),
            "Review request parameters and retry.",
            started,
        )
    })
}

fn open_read_runtime(
    state: &ApiState,
    tool: &'static str,
    started: Instant,
) -> Result<RuntimeContext, ApiError> {
    RuntimeContext::open(RuntimeContextOptions {
        config_path: state.config_path_override.clone(),
        db_path: state.db_path_override.clone(),
        create: false,
        ..RuntimeContextOptions::read_only()
    })
    .map_err(|error| ApiError::from_fin_error(tool, error, started))
}

fn load_rules_config(
    state: &ApiState,
    tool: &'static str,
    started: Instant,
) -> Result<Option<LoadedConfig>, ApiError> {
    match state.config_path_override.as_deref() {
        Some(path) => load_config(Some(path))
            .map(Some)
            .map_err(|error| ApiError::from_fin_error(tool, error, started)),
        None => Ok(load_config(None).ok()),
    }
}

fn parse_cursor_token(
    token: &str,
    tool: &'static str,
    started: Instant,
) -> Result<TransactionCursor, ApiError> {
    serde_json::from_str(token).map_err(|error| {
        ApiError::bad_request(
            tool,
            "INVALID_INPUT",
            format!("Invalid transaction cursor token: {error}"),
            "Pass the exact nextCursorToken returned by the previous page.",
            started,
        )
    })
}

fn serialize_cursor_token(cursor: &TransactionCursor) -> Result<String, String> {
    serde_json::to_string(cursor)
        .map_err(|error| format!("failed to serialize transaction cursor: {error}"))
}

fn ensure_group_exists(
    config: &fin_sdk::config::FinConfig,
    group_id: &str,
    tool: &'static str,
    started: Instant,
) -> Result<(), ApiError> {
    if config
        .group_ids()
        .iter()
        .any(|candidate| candidate == group_id)
    {
        Ok(())
    } else {
        Err(ApiError::not_found(
            tool,
            format!("group \"{group_id}\" not found"),
            "Call GET /v1/config/show to inspect configured groups.",
            started,
        ))
    }
}

fn ensure_account_exists<'a>(
    config: &'a fin_sdk::config::FinConfig,
    account_id: &str,
    tool: &'static str,
    started: Instant,
) -> Result<&'a fin_sdk::config::AccountConfig, ApiError> {
    config.account_by_id(account_id).ok_or_else(|| {
        ApiError::not_found(
            tool,
            format!("account \"{account_id}\" not found"),
            "Call GET /v1/config/show or GET /v1/view/accounts to inspect available accounts.",
            started,
        )
    })
}

fn validate_year_month(value: &str, tool: &'static str, started: Instant) -> Result<(), ApiError> {
    let bytes = value.as_bytes();
    let valid = bytes.len() == 7
        && bytes[4] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..].iter().all(u8::is_ascii_digit);
    if valid {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            tool,
            "INVALID_INPUT",
            format!("invalid year-month value: {value}"),
            "Use YYYY-MM, for example 2026-03.",
            started,
        ))
    }
}

fn build_balance_series_options(
    from: Option<String>,
    to: Option<String>,
    limit: usize,
    downsample_min_step_days: Option<u32>,
) -> BalanceSeriesQueryOptions {
    BalanceSeriesQueryOptions {
        from,
        to,
        limit,
        downsample_min_step_days,
    }
}

fn hierarchy_node_count(nodes: &[ExpenseHierarchyNode]) -> usize {
    nodes
        .iter()
        .map(|node| 1 + hierarchy_node_count(&node.children))
        .sum()
}

fn filter_expense_hierarchy_nodes(nodes: &[ExpenseHierarchyNode]) -> Vec<ExpenseHierarchyNode> {
    nodes
        .iter()
        .filter(|node| node.kind == ExpenseHierarchyNodeKind::Expense)
        .map(filter_expense_hierarchy_node)
        .collect()
}

fn filter_expense_hierarchy_node(node: &ExpenseHierarchyNode) -> ExpenseHierarchyNode {
    ExpenseHierarchyNode {
        account_id: node.account_id.clone(),
        name: node.name.clone(),
        kind: node.kind,
        total_minor: node.total_minor,
        share_of_parent_pct: node.share_of_parent_pct,
        share_of_root_pct: node.share_of_root_pct,
        children: node
            .children
            .iter()
            .filter(|child| child.kind == ExpenseHierarchyNodeKind::Expense)
            .map(filter_expense_hierarchy_node)
            .collect(),
    }
}

fn filter_dashboard_flow_graph(graph: FlowGraph) -> FlowGraph {
    let node_kind_by_id = graph
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.kind))
        .collect::<BTreeMap<_, _>>();
    let edges = graph
        .edges
        .into_iter()
        .filter(|edge| {
            let source_kind = node_kind_by_id.get(&edge.source_id).copied();
            let target_kind = node_kind_by_id.get(&edge.target_id).copied();
            !matches!(
                (source_kind, target_kind),
                (Some(FlowNodeKind::Asset), Some(FlowNodeKind::Asset))
            )
        })
        .collect::<Vec<_>>();
    let used_node_ids = edges
        .iter()
        .flat_map(|edge| [edge.source_id.clone(), edge.target_id.clone()])
        .collect::<std::collections::BTreeSet<_>>();
    let nodes = graph
        .nodes
        .into_iter()
        .filter(|node| used_node_ids.contains(&node.id))
        .collect::<Vec<_>>();

    FlowGraph {
        total_minor: edges.iter().map(|edge| edge.amount_minor).sum(),
        nodes,
        edges,
    }
}

fn selected_runway_groups(
    config: &fin_sdk::config::FinConfig,
    include: Option<&str>,
) -> Vec<String> {
    let include_groups = include
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    if include_groups.is_empty() {
        config.group_ids()
    } else {
        include_groups
    }
}

fn validated_selected_groups(
    config: &fin_sdk::config::FinConfig,
    include: Option<&str>,
    tool: &'static str,
    started: Instant,
) -> Result<Vec<String>, ApiError> {
    let groups = selected_runway_groups(config, include);
    for group_id in &groups {
        ensure_group_exists(config, group_id, tool, started)?;
    }
    Ok(groups)
}

fn consolidate_runway_series(
    runtime: &RuntimeContext,
    groups: &[String],
    from: Option<&str>,
    to: Option<&str>,
    started: Instant,
) -> Result<Vec<RunwayPoint>, ApiError> {
    let mut merged = BTreeMap::<String, (i64, i64, i64)>::new();
    for group_id in groups {
        let series = report_runway(runtime.connection(), runtime.config(), group_id, from, to)
            .map_err(|error| ApiError::from_fin_error("report.runway", error, started))?;
        for point in series {
            let slot = merged.entry(point.date).or_insert((0, 0, 0));
            slot.0 += point.balance_minor;
            slot.1 += point.burn_rate_minor;
            slot.2 += point.median_expense_minor;
        }
    }

    Ok(merged
        .into_iter()
        .map(
            |(date, (balance_minor, burn_rate_minor, median_expense_minor))| RunwayPoint {
                runway_months: if burn_rate_minor <= 0 {
                    999.0
                } else {
                    (balance_minor as f64) / (burn_rate_minor as f64)
                },
                date,
                balance_minor,
                burn_rate_minor,
                median_expense_minor,
            },
        )
        .collect())
}

fn success<T: Serialize>(
    tool: &'static str,
    data: T,
    started: Instant,
    extras: MetaExtras,
) -> Json<SuccessEnvelope<T>> {
    Json(SuccessEnvelope::new(data, meta(tool, started, extras)))
}

fn meta(tool: &str, started: Instant, extras: MetaExtras) -> EnvelopeMeta {
    let mut meta = EnvelopeMeta::new(tool, elapsed_ms(started));
    meta.count = extras.count;
    meta.total = extras.total;
    meta.has_more = extras.has_more;
    meta
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

const fn default_discover_min() -> usize {
    2
}

const fn default_discover_limit() -> usize {
    500
}

const fn default_dashboard_kpi_months() -> usize {
    24
}

const fn default_dashboard_insight_months() -> usize {
    6
}

const fn default_report_months() -> usize {
    12
}

const fn default_page_limit() -> usize {
    50
}

const fn default_series_limit() -> usize {
    0
}

const fn default_projection_months() -> usize {
    24
}

const fn default_projection_minimum_burn_ratio() -> f64 {
    0.6
}

fn default_categories_mode() -> String {
    "breakdown".to_owned()
}

const fn default_categories_months() -> usize {
    3
}

const fn default_categories_limit() -> usize {
    10
}

const fn default_audit_months() -> usize {
    6
}

const fn default_audit_limit() -> usize {
    50
}

const fn default_sort_field() -> TransactionSortField {
    TransactionSortField::PostedAt
}

const fn default_sort_direction() -> SortDirection {
    SortDirection::Desc
}

const fn default_rollup_mode() -> RollupMode {
    RollupMode::MonthlyAverage
}
