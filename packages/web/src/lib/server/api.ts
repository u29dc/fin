import path from "node:path";

import {
	fallbackGroupMetadata,
	fallbackGroups,
	type ConnectionState,
	type GroupId,
	type GroupMeta,
} from "$lib/server/skeleton";

export type ApiMeta = {
	tool: string;
	elapsed: number;
	count?: number;
	total?: number;
	hasMore?: boolean;
};

export type ApiErrorPayload = {
	code: string;
	message: string;
	hint: string;
};

export type ApiSuccessEnvelope<T> = {
	ok: true;
	data: T;
	meta: ApiMeta;
};

export type ApiErrorEnvelope = {
	ok: false;
	error: ApiErrorPayload;
	meta: ApiMeta;
};

export type ApiEnvelope<T> = ApiSuccessEnvelope<T> | ApiErrorEnvelope;

export type ConfigGroup = {
	id: string;
	label: string;
	icon?: string | null;
	taxType: string;
	expenseReserveMonths: number;
};

export type ConfigAccount = {
	id: string;
	provider: string;
	label?: string | null;
	subtype?: string | null;
};

export type ConfigShowData = {
	groups: ConfigGroup[];
	accounts: Record<string, ConfigAccount[]>;
	financial: Record<string, unknown>;
	configPath: string;
};

export type CheckStatus = "ok" | "missing" | "invalid";
export type Severity = "blocking" | "degraded" | "info";
export type HealthStatus = "ready" | "degraded" | "blocked";

export type HealthCheck = {
	id: string;
	label: string;
	status: CheckStatus;
	severity: Severity;
	detail?: string | null;
	fix?: string[] | null;
};

export type HealthSummary = {
	ok: number;
	blocking: number;
	degraded: number;
};

export type HealthReport = {
	status: HealthStatus;
	checks: HealthCheck[];
	summary: HealthSummary;
};

export type AccountBalanceRow = {
	id: string;
	name: string;
	account_type: string;
	balance_minor: number | null;
	updated_at: string | null;
};

export type ViewAccountsData = {
	accounts: AccountBalanceRow[];
	total: number;
};

export type MonthlyCashflowPoint = {
	month: string;
	income_minor: number;
	expense_minor: number;
	net_minor: number;
	savings_rate_pct: number | null;
	rolling_median_expense_minor: number | null;
	expense_deviation_ratio: number | null;
	is_anomaly: boolean;
};

export type CashflowTotals = {
	income_minor: number;
	expense_minor: number;
	net_minor: number;
};

export type CashflowReport = {
	series: MonthlyCashflowPoint[];
	totals: CashflowTotals;
};

export type DashboardKpisData = {
	groupId: string;
	groupLabel: string;
	months: number;
	kpis: {
		current_month: string;
		current_partial_month: MonthlyCashflowPoint | null;
		last_full_month: MonthlyCashflowPoint | null;
		previous_full_month: MonthlyCashflowPoint | null;
		trailing_average_net_minor: number | null;
		median_spend_minor: number | null;
		short_term_trend: ShortTermTrend | null;
		anomaly_count_last_12_months: number;
		recent_anomaly_months: string[];
	};
};

export type ShortTermTrend = "positive" | "negative" | "flat";

export type GroupSummary = {
	label: string;
	net_worth_minor: number;
	latest_runway_months: number | null;
	latest_health_minor: number | null;
	latest_available_minor: number | null;
	last_full_month_net_minor: number | null;
	trailing_average_net_minor: number | null;
	median_spend_minor: number | null;
	short_term_trend: ShortTermTrend | null;
	anomaly_count_last_12_months: number;
};

export type SummaryReport = {
	generated_at: string;
	period_months: number;
	groups: Record<string, GroupSummary>;
	consolidated: {
		net_worth_minor: number;
		balance_sheet: {
			assets: number;
			liabilities: number;
			equity: number;
			income: number;
			expenses: number;
			net_worth: number;
			net_income: number;
		};
	};
};

export type RunwayPoint = {
	date: string;
	runway_months: number;
	balance_minor: number;
	burn_rate_minor: number;
	median_expense_minor: number;
};

export type RunwayReport = {
	series: RunwayPoint[];
	latest: RunwayPoint | null;
	groups: string[];
};

export type AllocationBucket =
	| "available_cash"
	| "expense_reserve"
	| "tax_reserve"
	| "emergency_fund"
	| "savings"
	| "investment"
	| "other";

export type AllocationSegment = {
	bucket: AllocationBucket;
	label: string;
	amount_minor: number;
	share_pct: number;
	account_ids: string[];
	derived: boolean;
};

export type DashboardAllocationBasis = "personal_buffer" | "reserve_composition";

export type DashboardAllocationSummary = {
	basis: DashboardAllocationBasis;
	balance_basis_minor: number;
	display_total_minor: number;
	available_minor: number;
	expense_reserve_minor: number;
	expense_reserve_display_minor: number;
	tax_reserve_minor: number;
	emergency_fund_minor: number;
	savings_minor: number;
	investment_minor: number;
	shortfall_minor: number;
	under_reserved: boolean;
	segments: AllocationSegment[];
};

export type GroupAllocationSnapshot = {
	group_id: string;
	group_label: string;
	net_total_minor: number;
	positive_total_minor: number;
	account_segments: AllocationSegment[];
	dashboard: DashboardAllocationSummary;
};

export type DashboardAllocationData = {
	reportingMonth: string;
	snapshot: GroupAllocationSnapshot;
};

export type FlowNodeKind = "income" | "asset" | "expense";

export type FlowNode = {
	id: string;
	label: string;
	kind: FlowNodeKind;
};

export type FlowEdge = {
	source_id: string;
	target_id: string;
	amount_minor: number;
	share_of_total_pct: number;
	share_of_source_pct: number;
};

export type FlowGraph = {
	total_minor: number;
	nodes: FlowNode[];
	edges: FlowEdge[];
};

export type DashboardFlowData = {
	groupId: string;
	months: number;
	mode: string;
	graph: FlowGraph;
};

export type ExpenseHierarchyNodeKind = "expense" | "transfer";

export type ExpenseHierarchyNode = {
	account_id: string;
	name: string;
	kind: ExpenseHierarchyNodeKind;
	total_minor: number;
	share_of_parent_pct: number;
	share_of_root_pct: number;
	children: ExpenseHierarchyNode[];
};

export type DashboardHierarchyData = {
	groupId: string;
	months: number;
	mode: string;
	totalMinor: number;
	nodes: ExpenseHierarchyNode[];
};

export type DailyBalancePoint = {
	date: string;
	balance_minor: number;
};

export type DashboardBalanceScopeKind = "all_assets" | "group" | "account";

export type DashboardBalanceData = {
	scopeKind: DashboardBalanceScopeKind;
	scopeId: string;
	scopeLabel: string;
	series: DailyBalancePoint[];
};

export type ContributionPoint = {
	date: string;
	contributions_minor: number;
};

export type DashboardContributionData = {
	accountId: string;
	accountLabel: string;
	series: ContributionPoint[];
};

export type ProjectionScopeKind = "group" | "consolidated";
export type ProjectionScenarioKind = "current_burn" | "minimum_burn";

export type RunwayThresholds = {
	warning_minor: number | null;
	threshold_minor: number | null;
};

export type RunwayThresholdCrossing = {
	month_index: number;
	date: string;
	balance_minor: number;
};

export type RunwayProjectionPoint = {
	month_index: number;
	date: string;
	balance_minor: number;
};

export type RunwayProjectionScenario = {
	kind: ProjectionScenarioKind;
	label: string;
	burn_rate_minor: number;
	is_net_positive: boolean;
	zero_balance_crossing: RunwayThresholdCrossing | null;
	warning_crossing: RunwayThresholdCrossing | null;
	threshold_crossing: RunwayThresholdCrossing | null;
	points: RunwayProjectionPoint[];
};

export type RunwayProjectionAssumptions = {
	as_of_date: string;
	projection_months: number;
	trailing_outflow_window_months: number;
	burn_rate_method: string;
	minimum_burn_ratio: number;
	full_months_only: boolean;
	include_as_of_month_in_history: boolean;
};

export type RunwayProjectionReport = {
	scope_kind: ProjectionScopeKind;
	scope_id: string;
	liquid_balance_minor: number;
	current_burn_minor: number;
	minimum_burn_minor: number;
	median_monthly_expense_minor: number;
	thresholds: RunwayThresholds;
	assumptions: RunwayProjectionAssumptions;
	scenarios: RunwayProjectionScenario[];
};

export type DashboardProjectionData = {
	groups: string[];
	report: RunwayProjectionReport;
};

export type ShellState = {
	availableGroups: GroupId[];
	groupMetadata: Record<GroupId, GroupMeta>;
	connection: ConnectionState;
	config: ConfigShowData | null;
	health: HealthReport | null;
};

export type ApiTransport =
	| {
			kind: "unix";
			origin: string;
			socketPath: string;
	  }
	| {
			kind: "tcp";
			origin: string;
	  };

export type EnvLike = Record<string, string | undefined>;

type BunRequestInit = RequestInit & {
	unix?: string;
};

type FetchLike = (input: string | URL | Request, init?: BunRequestInit) => Promise<Response>;

type QueryValue = string | number | boolean | null | undefined;

export type BalanceQuery = {
	group?: string;
	account?: string;
	from?: string;
	to?: string;
	limit?: number;
	downsampleMinStepDays?: number;
};

export type ProjectionQuery = {
	group?: string;
	consolidated?: boolean;
	include?: string;
	months?: number;
	minimumBurnRatio?: number;
	as_of?: string;
	trailingOutflowWindowMonths?: number;
};

export type FinApiClient = {
	readonly transport: ApiTransport;
	get<T>(pathname: string, query?: Record<string, QueryValue>): Promise<T>;
	configShow(): Promise<ConfigShowData>;
	health(): Promise<HealthReport>;
	viewAccounts(group?: string): Promise<ViewAccountsData>;
	reportSummary(months?: number): Promise<SummaryReport>;
	reportCashflow(group: string, months?: number): Promise<CashflowReport>;
	reportRunway(group: string, months?: number): Promise<RunwayReport>;
	dashboardKpis(group: string, months?: number): Promise<DashboardKpisData>;
	dashboardAllocation(group: string, month?: string): Promise<DashboardAllocationData>;
	dashboardFlow(group: string, months?: number): Promise<DashboardFlowData>;
	dashboardHierarchy(group: string, months?: number): Promise<DashboardHierarchyData>;
	dashboardBalances(query: BalanceQuery): Promise<DashboardBalanceData>;
	dashboardContributions(account: string, limit?: number): Promise<DashboardContributionData>;
	dashboardProjection(query: ProjectionQuery): Promise<DashboardProjectionData>;
};

const DEFAULT_UNIX_ORIGIN = "http://localhost";
const DEFAULT_TCP_ORIGIN = "http://127.0.0.1:7414";
const PREFERRED_GROUP_ORDER = ["personal", "joint", "business"];

export class FinApiError extends Error {
	readonly code: string;
	readonly status: number;
	readonly hint: string;
	readonly meta: ApiMeta | null;

	constructor(message: string, options: { code: string; status: number; hint: string; meta?: ApiMeta | null }) {
		super(message);
		this.name = "FinApiError";
		this.code = options.code;
		this.status = options.status;
		this.hint = options.hint;
		this.meta = options.meta ?? null;
	}
}

export function resolveFinHome(env: EnvLike = process.env): string {
	if (env.FIN_HOME) {
		return env.FIN_HOME;
	}
	if (env.TOOLS_HOME) {
		return path.join(env.TOOLS_HOME, "fin");
	}
	const home = env.HOME ?? process.env.HOME ?? ".";
	return path.join(home, ".tools", "fin");
}

export function resolveApiTransport(env: EnvLike = process.env): ApiTransport {
	const explicitTransport = env.FIN_API_TRANSPORT?.trim().toLowerCase();
	const explicitBaseUrl = normalizeUrl(env.FIN_API_BASE_URL);
	if (explicitTransport === "tcp" || explicitBaseUrl) {
		return {
			kind: "tcp",
			origin: explicitBaseUrl ?? DEFAULT_TCP_ORIGIN,
		};
	}

	return {
		kind: "unix",
		origin: normalizeUrl(env.FIN_API_ORIGIN) ?? DEFAULT_UNIX_ORIGIN,
		socketPath: env.FIN_API_SOCKET_PATH ?? path.join(resolveFinHome(env), "run", "fin-api.sock"),
	};
}

export function createFinApiClient(options?: { env?: EnvLike; fetch?: FetchLike }): FinApiClient {
	const transport = resolveApiTransport(options?.env);
	const fetchImpl: FetchLike = options?.fetch ?? ((input, init) => fetch(input, init));

	async function get<T>(pathname: string, query?: Record<string, QueryValue>): Promise<T> {
		const url = buildUrl(transport, pathname, query);
		const requestInit: BunRequestInit = transport.kind === "unix" ? { unix: transport.socketPath } : {};
		const response = await fetchImpl(url, requestInit);
		const envelope = await parseEnvelope<T>(response);
		if (!response.ok || !envelope.ok) {
			throw toApiError(response.status, envelope);
		}
		return envelope.data;
	}

	return {
		transport,
		get,
		configShow() {
			return get<ConfigShowData>("/v1/config/show");
		},
		health() {
			return get<HealthReport>("/v1/health");
		},
		viewAccounts(group?: string) {
			return get<ViewAccountsData>("/v1/view/accounts", { group });
		},
		reportSummary(months?: number) {
			return get<SummaryReport>("/v1/report/summary", { months });
		},
		reportCashflow(group: string, months = 24) {
			return get<CashflowReport>("/v1/report/cashflow", { group, months });
		},
		reportRunway(group: string, months = 120) {
			return get<RunwayReport>("/v1/report/runway", { group, months });
		},
		dashboardKpis(group: string, months = 12) {
			return get<DashboardKpisData>("/v1/dashboard/kpis", { group, months });
		},
		dashboardAllocation(group: string, month?: string) {
			return get<DashboardAllocationData>("/v1/dashboard/allocation", { group, month });
		},
		dashboardFlow(group: string, months = 6) {
			return get<DashboardFlowData>("/v1/dashboard/flow", { group, months });
		},
		dashboardHierarchy(group: string, months = 6) {
			return get<DashboardHierarchyData>("/v1/dashboard/hierarchy", { group, months });
		},
		dashboardBalances(query: BalanceQuery) {
			return get<DashboardBalanceData>("/v1/dashboard/balances", query);
		},
		dashboardContributions(account: string, limit = 10_000) {
			return get<DashboardContributionData>("/v1/dashboard/contributions", { account, limit });
		},
		dashboardProjection(query: ProjectionQuery) {
			return get<DashboardProjectionData>("/v1/dashboard/projection", query);
		},
	};
}

export async function loadShellState(client: FinApiClient = createFinApiClient()): Promise<ShellState> {
	const [configResult, healthResult] = await Promise.allSettled([client.configShow(), client.health()]);
	const config = configResult.status === "fulfilled" ? configResult.value : null;
	const health = healthResult.status === "fulfilled" ? healthResult.value : null;
	const shellGroups = deriveGroups(config);

	return {
		availableGroups: shellGroups.availableGroups,
		groupMetadata: shellGroups.groupMetadata,
		connection: deriveConnectionState(client.transport, configResult, healthResult),
		config,
		health,
	};
}

function normalizeUrl(value: string | undefined): string | null {
	if (!value) {
		return null;
	}
	return value.endsWith("/") ? value.slice(0, -1) : value;
}

function buildUrl(transport: ApiTransport, pathname: string, query?: Record<string, QueryValue>): string {
	const url = new URL(pathname, transport.origin);
	for (const [key, value] of Object.entries(query ?? {})) {
		if (value === null || value === undefined || value === "") {
			continue;
		}
		url.searchParams.set(key, String(value));
	}
	return url.toString();
}

async function parseEnvelope<T>(response: Response): Promise<ApiEnvelope<T>> {
	const raw = await response.text();
	try {
		return JSON.parse(raw) as ApiEnvelope<T>;
	} catch {
		throw new FinApiError("fin-api returned invalid JSON", {
			code: "INVALID_RESPONSE",
			status: response.status,
			hint: raw.length > 0 ? raw.slice(0, 200) : "empty response body",
			meta: null,
		});
	}
}

function toApiError(status: number, envelope: ApiEnvelope<unknown>): FinApiError {
	if (envelope.ok) {
		return new FinApiError(`fin-api request failed with status ${status}`, {
			code: "HTTP_ERROR",
			status,
			hint: envelope.meta.tool,
			meta: envelope.meta,
		});
	}
	return new FinApiError(envelope.error.message, {
		code: envelope.error.code,
		status,
		hint: envelope.error.hint,
		meta: envelope.meta,
	});
}

function deriveGroups(config: ConfigShowData | null): Pick<ShellState, "availableGroups" | "groupMetadata"> {
	if (!config) {
		return {
			availableGroups: fallbackGroups,
			groupMetadata: fallbackGroupMetadata,
		};
	}

	const orderedGroups = [...config.groups].sort((left, right) => {
		const leftIndex = preferredGroupIndex(left.id);
		const rightIndex = preferredGroupIndex(right.id);
		return leftIndex - rightIndex || left.label.localeCompare(right.label);
	});

	return {
		availableGroups: orderedGroups.map((group) => group.id),
		groupMetadata: Object.fromEntries(
			orderedGroups.map((group) => [
				group.id,
				{
					label: group.label,
					icon: group.icon ?? "wallet",
				},
			]),
		) as Record<GroupId, GroupMeta>,
	};
}

function preferredGroupIndex(groupId: string): number {
	const index = PREFERRED_GROUP_ORDER.indexOf(groupId);
	return index === -1 ? PREFERRED_GROUP_ORDER.length : index;
}

function deriveConnectionState(
	transport: ApiTransport,
	configResult: PromiseSettledResult<ConfigShowData>,
	healthResult: PromiseSettledResult<HealthReport>,
): ConnectionState {
	const transportLabel = describeTransport(transport);

	if (healthResult.status === "fulfilled") {
		const health = healthResult.value;
		if (health.status === "ready") {
			return {
				loading: false,
				error: null,
				detail: `Connected to fin-api via ${transportLabel}.`,
			};
		}
		return {
			loading: false,
			error: `api ${health.status}`,
			detail: `${summarizeHealthIssues(health)} via ${transportLabel}.`,
		};
	}

	if (configResult.status === "fulfilled") {
		return {
			loading: false,
			error: "health unavailable",
			detail: `${formatReason(healthResult.reason)} via ${transportLabel}.`,
		};
	}

	return {
		loading: false,
		error: "api unavailable",
		detail: `${formatReason(configResult.reason)} via ${transportLabel}.`,
	};
}

function describeTransport(transport: ApiTransport): string {
	if (transport.kind === "tcp") {
		return transport.origin;
	}
	return `unix socket ${transport.socketPath}`;
}

function summarizeHealthIssues(report: HealthReport): string {
	const failingChecks = report.checks.filter((check) => check.status !== "ok");
	if (failingChecks.length === 0) {
		return `Health status ${report.status}`;
	}
	const first = failingChecks[0];
	if (!first) {
		return `Health status ${report.status}`;
	}
	const detail = first.detail ? `: ${first.detail}` : "";
	return `${first.label}${detail}`;
}

function formatReason(reason: unknown): string {
	if (reason instanceof FinApiError) {
		return reason.message;
	}
	if (reason instanceof Error) {
		return reason.message;
	}
	return "Unknown fin-api error";
}
