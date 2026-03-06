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
    AccountBalanceRow, BalanceSheet, ConfigShowData, ConfigValidationResult, DescriptionSummary,
    EnvelopeMeta, ErrorEnvelope, ErrorPayload, FinError, FinSdkError, GlobalFlag,
    HealthCheckOptions, HealthReport, JournalEntryRow, LedgerQueryOptions, RuntimeContext,
    RuntimeContextOptions, SDK_VERSION, SortDirection, SuccessEnvelope, ToolMeta,
    TransactionCursor, TransactionDetail, TransactionListRow, TransactionPageQuery,
    TransactionSortField, ValidationError, build_config_show, discover_descriptions,
    discover_unmapped_descriptions, get_balance_sheet, global_flags, ledger_entry_count,
    load_transaction_detail, query_transactions_page, run_health_checks, sdk_banner, tool_registry,
    validate_config, view_accounts, view_ledger,
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

const fn default_page_limit() -> usize {
    50
}

const fn default_sort_field() -> TransactionSortField {
    TransactionSortField::PostedAt
}

const fn default_sort_direction() -> SortDirection {
    SortDirection::Desc
}
