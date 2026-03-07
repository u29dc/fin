import {
	chooseDownsampleStepDays,
	downsampleByMinStep,
	toUtcMsFromIsoDate,
	type AnnualReturns,
	type BalancePoint,
	type CashflowPoint,
	type InvestmentPoint,
} from "$lib/charts/utils";
import {
	createFinApiClient,
	loadShellState,
	type ConfigAccount,
	type ConfigShowData,
	type DashboardAllocationData,
	type DashboardBalanceData,
	type DashboardContributionData,
	type DashboardFlowData,
	type DashboardHierarchyData,
	type DashboardKpisData,
	type FinApiClient,
	type MonthlyCashflowPoint,
	type ShellState,
	type SummaryReport,
	type ViewAccountsData,
	type RunwayReport,
	type CashflowReport,
} from "$lib/server/api";
import { resolveGroup, type ConnectionState, type GroupId, type GroupMeta } from "$lib/server/skeleton";

const DAY_MS = 86_400_000;
const CASHFLOW_MONTHS = 240;
const SUMMARY_MONTHS = 12;
const FLOW_MONTHS = 6;
const HIERARCHY_MONTHS = 6;
const SERIES_LIMIT = 10_000;

export type DashboardAccount = {
	id: string;
	name: string;
	groupId: string;
	provider: string;
	subtype: string | null;
	accountType: string;
	latestBalance: {
		chartAccountId: string;
		date: string | null;
		balanceMinor: number | null;
	};
};

export type DashboardGroupConfig = {
	label: string;
	accountIds: string[];
	icon: string;
};

export type DashboardAccountGroupConfig = {
	label: string;
	accounts: Array<{ id: string; label: string }>;
};

export type DashboardUiConfig = {
	groups: Record<string, DashboardGroupConfig>;
	accountGroupConfig: Record<string, DashboardAccountGroupConfig>;
	groupColumnOrder: string[];
	groupMetadata: Record<string, GroupMeta>;
};

export type DashboardFinanceConfig = {
	investmentProjectionAnnualReturns: AnnualReturns | null;
};

export type DashboardAllocationSegment = {
	bucket: string;
	label: string;
	amountMinor: number;
	sharePct: number;
	accountIds: string[];
	derived: boolean;
};

export type DashboardAllocationSnapshot = {
	groupId: string;
	groupLabel: string;
	netTotalMinor: number;
	positiveTotalMinor: number;
	dashboard: {
		basis: string;
		balanceBasisMinor: number;
		displayTotalMinor: number;
		availableMinor: number;
		expenseReserveMinor: number;
		expenseReserveDisplayMinor: number;
		taxReserveMinor: number;
		emergencyFundMinor: number;
		savingsMinor: number;
		investmentMinor: number;
		shortfallMinor: number;
		underReserved: boolean;
		segments: DashboardAllocationSegment[];
	};
};

export type SankeyFlowData = {
	nodes: Array<{ id: string; label: string; category: "income" | "asset" | "expense" }>;
	links: Array<{ source: string; target: string; value: number }>;
};

export type ExpenseNode = {
	accountId: string;
	name: string;
	kind: "expense" | "transfer";
	totalMinor: number;
	shareOfParentPct: number;
	shareOfRootPct: number;
	children: ExpenseNode[];
};

export type DashboardRunwaySummary = {
	runwayMonths: number;
	isNetPositive: boolean;
	medianExpenseMinor: number | null;
};

export type DashboardGroupSummary = {
	label: string;
	netWorthMinor: number;
	latestRunwayMonths: number | null;
	latestHealthMinor: number | null;
	latestAvailableMinor: number | null;
	lastFullMonthNetMinor: number | null;
	trailingAverageNetMinor: number | null;
	medianSpendMinor: number | null;
	shortTermTrend: "positive" | "negative" | "flat" | null;
	anomalyCountLast12Months: number;
	recentAnomalyMonths: string[];
};

export type DashboardPageData = {
	availableGroups: GroupId[];
	groupMetadata: Record<GroupId, GroupMeta>;
	connection: ConnectionState;
	config: {
		finance: DashboardFinanceConfig;
		ui: DashboardUiConfig;
	};
	accounts: DashboardAccount[];
	accountBalanceSeries: Record<string, BalancePoint[]>;
	accountContributionSeries: Record<string, InvestmentPoint[]>;
	groupCashflowSeries: Record<string, CashflowPoint[]>;
	groupRunway: Record<string, DashboardRunwaySummary | null>;
	groupAllocationSnapshots: Record<string, DashboardAllocationSnapshot | null>;
	groupCashFlowData: Record<string, SankeyFlowData>;
	groupExpenseHierarchy: Record<string, ExpenseNode[]>;
	groupSummary: Record<string, DashboardGroupSummary>;
	initialGroup: GroupId;
};

export async function loadDashboardPageData(options: {
	url: URL;
	client?: FinApiClient;
}): Promise<DashboardPageData> {
	const client = options.client ?? createFinApiClient();
	const shell = await loadShellState(client);
	const initialGroup = resolveGroup(options.url, shell.availableGroups);
	const ui = buildUiConfig(shell);
	const finance = buildFinanceConfig(shell.config);
	const base = createEmptyPageData(shell, initialGroup, ui, finance);

	if (!shell.config) {
		return base;
	}

	const [accountsResult, summaryResult, groupPayloads] = await Promise.all([
		safeFetch(() => client.viewAccounts()),
		safeFetch(() => client.reportSummary(SUMMARY_MONTHS)),
		loadGroupPayloads(client, shell.availableGroups),
	]);

	const accountConfigs = flattenConfiguredAccounts(shell.config, shell.availableGroups);
	const accounts = mergeAccounts(accountConfigs, accountsResult);
	const [accountBalanceSeries, accountContributionSeries] = await Promise.all([
		loadAccountBalanceSeries(client, accountConfigs),
		loadAccountContributionSeries(client, accountConfigs),
	]);

	return {
		...base,
		accounts,
		accountBalanceSeries,
		accountContributionSeries,
		groupCashflowSeries: mapCashflowSeries(shell.availableGroups, groupPayloads.cashflowByGroup),
		groupRunway: mapRunwaySummary(shell.availableGroups, groupPayloads.runwayByGroup),
		groupAllocationSnapshots: mapAllocationSnapshots(shell.availableGroups, groupPayloads.allocationByGroup),
		groupCashFlowData: mapFlowData(shell.availableGroups, groupPayloads.flowByGroup),
		groupExpenseHierarchy: mapHierarchyData(shell.availableGroups, groupPayloads.hierarchyByGroup),
		groupSummary: mapGroupSummary(shell.availableGroups, summaryResult, groupPayloads.kpisByGroup, accounts),
	};
}

async function loadGroupPayloads(client: FinApiClient, groupIds: readonly string[]) {
	const [cashflowByGroup, runwayByGroup, kpisByGroup, allocationByGroup, flowByGroup, hierarchyByGroup] =
		await Promise.all([
			loadGroupRecord(groupIds, (groupId) => client.reportCashflow(groupId, CASHFLOW_MONTHS)),
			loadGroupRecord(groupIds, (groupId) => client.reportRunway(groupId)),
			loadGroupRecord(groupIds, (groupId) => client.dashboardKpis(groupId, SUMMARY_MONTHS)),
			loadGroupRecord(groupIds, (groupId) => client.dashboardAllocation(groupId)),
			loadGroupRecord(groupIds, (groupId) => client.dashboardFlow(groupId, FLOW_MONTHS)),
			loadGroupRecord(groupIds, (groupId) => client.dashboardHierarchy(groupId, HIERARCHY_MONTHS)),
		]);

	return {
		cashflowByGroup,
		runwayByGroup,
		kpisByGroup,
		allocationByGroup,
		flowByGroup,
		hierarchyByGroup,
	};
}

async function loadGroupRecord<T>(
	groupIds: readonly string[],
	fetcher: (groupId: string) => Promise<T>,
): Promise<Record<string, T | null>> {
	const settled = await Promise.allSettled(groupIds.map(async (groupId) => [groupId, await fetcher(groupId)] as const));
	const entries: Array<[string, T | null]> = [];

	for (const result of settled) {
		if (result.status === "fulfilled") {
			entries.push([result.value[0], result.value[1]]);
		}
	}

	for (const groupId of groupIds) {
		if (!entries.some(([id]) => id === groupId)) {
			entries.push([groupId, null]);
		}
	}

	return Object.fromEntries(entries);
}

async function safeFetch<T>(fetcher: () => Promise<T>): Promise<T | null> {
	try {
		return await fetcher();
	} catch {
		return null;
	}
}

async function loadAccountBalanceSeries(
	client: FinApiClient,
	accountConfigs: ConfiguredAccount[],
): Promise<Record<string, BalancePoint[]>> {
	const settled = await Promise.allSettled(
		accountConfigs.map(async (account) => {
			const payload = await client.dashboardBalances({ account: account.id, limit: SERIES_LIMIT });
			return [account.id, mapBalanceSeries(payload)] as const;
		}),
	);

	return Object.fromEntries(
		accountConfigs.map((account) => {
			const result = settled.find((entry) => entry.status === "fulfilled" && entry.value[0] === account.id);
			if (result?.status === "fulfilled") {
				return [account.id, result.value[1]];
			}
			return [account.id, [] as BalancePoint[]];
		}),
	);
}

async function loadAccountContributionSeries(
	client: FinApiClient,
	accountConfigs: ConfiguredAccount[],
): Promise<Record<string, InvestmentPoint[]>> {
	const investmentAccounts = accountConfigs.filter((account) => isInvestmentAccount(account.config));
	const settled = await Promise.allSettled(
		investmentAccounts.map(async (account) => {
			const payload = await client.dashboardContributions(account.id, SERIES_LIMIT);
			return [account.id, mapContributionSeries(payload)] as const;
		}),
	);

	return Object.fromEntries(
		settled.flatMap((result) => {
			if (result.status !== "fulfilled") {
				return [];
			}
			return [[result.value[0], result.value[1]] as const];
		}),
	);
}

function buildUiConfig(shell: ShellState): DashboardUiConfig {
	const config = shell.config;
	const groups = Object.fromEntries(
		shell.availableGroups.map((groupId) => {
			const configGroup = config?.groups.find((group) => group.id === groupId);
			const label = shell.groupMetadata[groupId]?.label ?? configGroup?.label ?? fallbackLabel(groupId);
			const icon = shell.groupMetadata[groupId]?.icon ?? configGroup?.icon ?? "wallet";
			const groupAccounts = config?.accounts[groupId] ?? [];
			return [
				groupId,
				{
					label,
					accountIds: groupAccounts.map((account) => account.id),
					icon,
				},
			] as const;
		}),
	);

	const accountGroupConfig = Object.fromEntries(
		shell.availableGroups.map((groupId) => {
			const meta = shell.groupMetadata[groupId];
			const accounts = config?.accounts[groupId] ?? [];
			return [
				groupId,
				{
					label: `${meta?.label ?? fallbackLabel(groupId)} Accounts`,
					accounts: accounts.map((account) => ({ id: account.id, label: account.label ?? fallbackAccountLabel(account.id) })),
				},
			] as const;
		}),
	);

	return {
		groups,
		accountGroupConfig,
		groupColumnOrder: [...shell.availableGroups],
		groupMetadata: shell.groupMetadata,
	};
}

function buildFinanceConfig(config: ConfigShowData | null): DashboardFinanceConfig {
	const returns = config?.financial["investment_projection_annual_returns"];
	if (!isAnnualReturns(returns)) {
		return { investmentProjectionAnnualReturns: null };
	}
	return {
		investmentProjectionAnnualReturns: {
			low: returns.low,
			mid: returns.mid,
			high: returns.high,
		},
	};
}

function createEmptyPageData(
	shell: ShellState,
	initialGroup: GroupId,
	ui: DashboardUiConfig,
	finance: DashboardFinanceConfig,
): DashboardPageData {
	return {
		availableGroups: [...shell.availableGroups],
		groupMetadata: shell.groupMetadata,
		connection: shell.connection,
		config: {
			finance,
			ui,
		},
		accounts: [],
		accountBalanceSeries: {},
		accountContributionSeries: {},
		groupCashflowSeries: Object.fromEntries(shell.availableGroups.map((groupId) => [groupId, []])) as Record<string, CashflowPoint[]>,
		groupRunway: Object.fromEntries(shell.availableGroups.map((groupId) => [groupId, null])) as Record<
			string,
			DashboardRunwaySummary | null
		>,
		groupAllocationSnapshots: Object.fromEntries(shell.availableGroups.map((groupId) => [groupId, null])) as Record<
			string,
			DashboardAllocationSnapshot | null
		>,
		groupCashFlowData: Object.fromEntries(
			shell.availableGroups.map((groupId) => [groupId, { nodes: [], links: [] }]),
		) as Record<string, SankeyFlowData>,
		groupExpenseHierarchy: Object.fromEntries(shell.availableGroups.map((groupId) => [groupId, []])) as Record<
			string,
			ExpenseNode[]
		>,
		groupSummary: {},
		initialGroup,
	};
}

type ConfiguredAccount = {
	groupId: string;
	config: ConfigAccount;
	id: string;
};

function flattenConfiguredAccounts(config: ConfigShowData, orderedGroups: readonly string[]): ConfiguredAccount[] {
    const entries = orderedGroups.flatMap((groupId) =>
        (config.accounts[groupId] ?? []).map((account) => ({ groupId, config: account, id: account.id })),
    );
    const groupOrder = new Map(orderedGroups.map((groupId, index) => [groupId, index]));
    return entries.sort((left, right) => {
        const leftGroupIndex = groupOrder.get(left.groupId) ?? Number.MAX_SAFE_INTEGER;
        const rightGroupIndex = groupOrder.get(right.groupId) ?? Number.MAX_SAFE_INTEGER;
        if (leftGroupIndex !== rightGroupIndex) {
            return leftGroupIndex - rightGroupIndex;
        }
        return left.id.localeCompare(right.id);
    });
}

function mergeAccounts(accountConfigs: ConfiguredAccount[], accountsData: ViewAccountsData | null): DashboardAccount[] {
	const accountRows = new Map(accountsData?.accounts.map((account) => [account.id, account]) ?? []);
	return accountConfigs.map((account) => {
		const latest = accountRows.get(account.id);
		return {
			id: account.id,
			name: account.config.label ?? latest?.name ?? fallbackAccountLabel(account.id),
			groupId: account.groupId,
			provider: account.config.provider,
			subtype: account.config.subtype ?? null,
			accountType: latest?.account_type ?? "asset",
			latestBalance: {
				chartAccountId: account.id,
				date: latest?.updated_at ?? null,
				balanceMinor: latest?.balance_minor ?? null,
			},
		};
	});
}

function mapCashflowSeries(
	groupIds: readonly string[],
	cashflowByGroup: Record<string, CashflowReport | null>,
): Record<string, CashflowPoint[]> {
	return Object.fromEntries(
		groupIds.map((groupId) => {
			const report = cashflowByGroup[groupId];
			return [groupId, report ? report.series.map(toCashflowPoint) : []];
		}),
	) as Record<string, CashflowPoint[]>;
}

function mapRunwaySummary(
	groupIds: readonly string[],
	runwayByGroup: Record<string, RunwayReport | null>,
): Record<string, DashboardRunwaySummary | null> {
	return Object.fromEntries(
		groupIds.map((groupId) => {
			const latest = runwayByGroup[groupId]?.latest;
			if (!latest) {
				return [groupId, null];
			}
			return [
				groupId,
				{
					runwayMonths: latest.runway_months,
					isNetPositive: latest.burn_rate_minor <= 0,
					medianExpenseMinor: latest.median_expense_minor,
				},
			] as const;
		}),
	) as Record<string, DashboardRunwaySummary | null>;
}

function mapAllocationSnapshots(
	groupIds: readonly string[],
	allocationByGroup: Record<string, DashboardAllocationData | null>,
): Record<string, DashboardAllocationSnapshot | null> {
	return Object.fromEntries(
		groupIds.map((groupId) => {
			const payload = allocationByGroup[groupId];
			if (!payload) {
				return [groupId, null];
			}
			return [groupId, mapAllocationSnapshot(payload)];
		}),
	) as Record<string, DashboardAllocationSnapshot | null>;
}

function mapFlowData(groupIds: readonly string[], flowByGroup: Record<string, DashboardFlowData | null>): Record<string, SankeyFlowData> {
	return Object.fromEntries(
		groupIds.map((groupId) => {
			const payload = flowByGroup[groupId];
			if (!payload) {
				return [groupId, { nodes: [], links: [] }];
			}
			return [groupId, mapSankeyFlowData(payload)];
		}),
	) as Record<string, SankeyFlowData>;
}

function mapHierarchyData(
	groupIds: readonly string[],
	hierarchyByGroup: Record<string, DashboardHierarchyData | null>,
): Record<string, ExpenseNode[]> {
	return Object.fromEntries(
		groupIds.map((groupId) => {
			const payload = hierarchyByGroup[groupId];
			return [groupId, payload ? payload.nodes.map(mapExpenseNode) : []];
		}),
	) as Record<string, ExpenseNode[]>;
}

function mapGroupSummary(
	groupIds: readonly string[],
	summary: SummaryReport | null,
	kpisByGroup: Record<string, DashboardKpisData | null>,
	accounts: DashboardAccount[],
): Record<string, DashboardGroupSummary> {
	const totalsByGroup = summarizeAccountBalances(accounts);
	return Object.fromEntries(
		groupIds.flatMap((groupId) => {
			const summaryGroup = summary?.groups[groupId];
			const kpis = kpisByGroup[groupId]?.kpis;
			const label = summaryGroup?.label ?? fallbackLabel(groupId);
			if (!summaryGroup && !kpis && totalsByGroup[groupId] === undefined) {
				return [];
			}
			return [
				[
					groupId,
					{
						label,
						netWorthMinor: summaryGroup?.net_worth_minor ?? totalsByGroup[groupId] ?? 0,
						latestRunwayMonths: summaryGroup?.latest_runway_months ?? null,
						latestHealthMinor: summaryGroup?.latest_health_minor ?? null,
						latestAvailableMinor: summaryGroup?.latest_available_minor ?? null,
						lastFullMonthNetMinor: summaryGroup?.last_full_month_net_minor ?? kpis?.last_full_month?.net_minor ?? null,
						trailingAverageNetMinor: summaryGroup?.trailing_average_net_minor ?? kpis?.trailing_average_net_minor ?? null,
						medianSpendMinor: summaryGroup?.median_spend_minor ?? kpis?.median_spend_minor ?? null,
						shortTermTrend: summaryGroup?.short_term_trend ?? kpis?.short_term_trend ?? null,
						anomalyCountLast12Months:
							summaryGroup?.anomaly_count_last_12_months ?? kpis?.anomaly_count_last_12_months ?? 0,
						recentAnomalyMonths: kpis?.recent_anomaly_months ?? [],
					},
				] as const,
			];
		}),
	) as Record<string, DashboardGroupSummary>;
}

function summarizeAccountBalances(accounts: DashboardAccount[]): Record<string, number> {
	const totals: Record<string, number> = {};
	for (const account of accounts) {
		const balanceMinor = account.latestBalance.balanceMinor ?? 0;
		totals[account.groupId] = (totals[account.groupId] ?? 0) + balanceMinor;
	}
	return totals;
}

function mapBalanceSeries(payload: DashboardBalanceData): BalancePoint[] {
	return downsampleDailySeries(
		payload.series.map((point) => ({
			date: point.date,
			balanceMinor: point.balance_minor,
		})),
	);
}

function mapContributionSeries(payload: DashboardContributionData): InvestmentPoint[] {
	return downsampleDailySeries(
		payload.series.map((point) => ({
			date: point.date,
			contributionsMinor: point.contributions_minor,
		})),
	);
}

function toCashflowPoint(point: MonthlyCashflowPoint): CashflowPoint {
	return {
		month: point.month,
		incomeMinor: point.income_minor,
		expenseMinor: point.expense_minor,
		netMinor: point.net_minor,
		savingsRatePct: point.savings_rate_pct,
		rollingMedianExpenseMinor: point.rolling_median_expense_minor,
		expenseDeviationRatio: point.expense_deviation_ratio,
	};
}

function mapAllocationSnapshot(payload: DashboardAllocationData): DashboardAllocationSnapshot {
	return {
		groupId: payload.snapshot.group_id,
		groupLabel: payload.snapshot.group_label,
		netTotalMinor: payload.snapshot.net_total_minor,
		positiveTotalMinor: payload.snapshot.positive_total_minor,
		dashboard: {
			basis: payload.snapshot.dashboard.basis,
			balanceBasisMinor: payload.snapshot.dashboard.balance_basis_minor,
			displayTotalMinor: payload.snapshot.dashboard.display_total_minor,
			availableMinor: payload.snapshot.dashboard.available_minor,
			expenseReserveMinor: payload.snapshot.dashboard.expense_reserve_minor,
			expenseReserveDisplayMinor: payload.snapshot.dashboard.expense_reserve_display_minor,
			taxReserveMinor: payload.snapshot.dashboard.tax_reserve_minor,
			emergencyFundMinor: payload.snapshot.dashboard.emergency_fund_minor,
			savingsMinor: payload.snapshot.dashboard.savings_minor,
			investmentMinor: payload.snapshot.dashboard.investment_minor,
			shortfallMinor: payload.snapshot.dashboard.shortfall_minor,
			underReserved: payload.snapshot.dashboard.under_reserved,
			segments: payload.snapshot.dashboard.segments.map((segment) => ({
				bucket: segment.bucket,
				label: segment.label,
				amountMinor: segment.amount_minor,
				sharePct: segment.share_pct,
				accountIds: segment.account_ids,
				derived: segment.derived,
			})),
		},
	};
}

function mapSankeyFlowData(payload: DashboardFlowData): SankeyFlowData {
	const nodeIds = new Set(payload.graph.nodes.map((node) => node.id));
	return {
		nodes: payload.graph.nodes.map((node) => ({
			id: node.id,
			label: node.label,
			category: node.kind,
		})),
		links: payload.graph.edges
			.map((edge) => {
				if (!nodeIds.has(edge.source_id) || !nodeIds.has(edge.target_id)) {
					return null;
				}
				return {
					source: edge.source_id,
					target: edge.target_id,
					value: edge.amount_minor,
				};
			})
			.filter((edge): edge is NonNullable<typeof edge> => edge !== null),
	};
}

function mapExpenseNode(node: DashboardHierarchyData["nodes"][number]): ExpenseNode {
	return {
		accountId: node.account_id,
		name: node.name,
		kind: node.kind,
		totalMinor: node.total_minor,
		shareOfParentPct: node.share_of_parent_pct,
		shareOfRootPct: node.share_of_root_pct,
		children: node.children.map(mapExpenseNode),
	};
}

function isInvestmentAccount(account: ConfigAccount): boolean {
	return account.subtype === "investment" || account.provider === "vanguard";
}

function fallbackLabel(groupId: string): string {
	return groupId.charAt(0).toUpperCase() + groupId.slice(1);
}

function fallbackAccountLabel(accountId: string): string {
	return accountId.split(":").at(-1) ?? accountId;
}

function isAnnualReturns(value: unknown): value is AnnualReturns {
	if (!value || typeof value !== "object") {
		return false;
	}
	const candidate = value as Record<string, unknown>;
	return [candidate.low, candidate.mid, candidate.high].every((entry) => typeof entry === "number");
}

function toDayIndex(date: string): number | null {
	const ms = toUtcMsFromIsoDate(date);
	return ms === null ? null : Math.floor(ms / DAY_MS);
}

function downsampleDailySeries<T extends { date: string }>(series: T[]): T[] {
	if (series.length <= 2) {
		return series;
	}

	const first = series[0];
	const last = series[series.length - 1];
	if (!first || !last) {
		return series;
	}

	const firstIndex = toDayIndex(first.date);
	const lastIndex = toDayIndex(last.date);
	if (firstIndex === null || lastIndex === null || lastIndex <= firstIndex) {
		return series;
	}

	const spanDays = Math.max(1, lastIndex - firstIndex);
	const step = chooseDownsampleStepDays(spanDays);
	return downsampleByMinStep(series, (point) => toDayIndex(point.date), step);
}
