import { chooseDownsampleStepDays, type BalancePoint, type ProjectionPoint } from "$lib/charts/utils";
import {
	createFinApiClient,
	loadShellState,
	type AccountBalanceRow,
	type ConfigShowData,
	type DashboardBalanceData,
	type DashboardProjectionData,
	type FinApiClient,
	type ShellState,
	type ViewAccountsData,
} from "$lib/server/api";
import { type ConnectionState, type GroupId, type GroupMeta } from "$lib/server/skeleton";

const DAY_MS = 86_400_000;
const SERIES_LIMIT = 10_000;
const PROJECTION_MONTHS = 24;
const TOTAL_SERIES_ID = "__all_assets__";

export type OverviewChartAccount = {
	id: string;
	label: string;
	groupId: string;
	provider: string;
	subtype: string | null;
	latestBalanceMinor: number | null;
	updatedAt: string | null;
};

export type OverviewProjectionCrossing = {
	monthIndex: number;
	date: string;
	balanceMinor: number;
};

export type OverviewProjectionScenario = {
	kind: "current_burn" | "minimum_burn";
	label: string;
	burnRateMinor: number;
	isNetPositive: boolean;
	zeroBalanceCrossing: OverviewProjectionCrossing | null;
	warningCrossing: OverviewProjectionCrossing | null;
	thresholdCrossing: OverviewProjectionCrossing | null;
	points: ProjectionPoint[];
};

export type OverviewProjection = {
	groups: string[];
	liquidBalanceMinor: number;
	currentBurnMinor: number;
	minimumBurnMinor: number;
	medianMonthlyExpenseMinor: number;
	thresholds: {
		warningMinor: number | null;
		thresholdMinor: number | null;
	};
	assumptions: {
		asOfDate: string;
		projectionMonths: number;
		trailingOutflowWindowMonths: number;
		burnRateMethod: string;
		minimumBurnRatio: number;
		fullMonthsOnly: boolean;
		includeAsOfMonthInHistory: boolean;
	};
	currentBurn: OverviewProjectionScenario | null;
	minimumBurn: OverviewProjectionScenario | null;
};

export type OverviewPageData = {
	availableGroups: GroupId[];
	groupMetadata: Record<GroupId, GroupMeta>;
	connection: ConnectionState;
	chartAccounts: OverviewChartAccount[];
	totalBalanceSeries: BalancePoint[];
	accountBalanceSeries: Record<string, BalancePoint[]>;
	projection: OverviewProjection | null;
	totalSeriesId: string;
};

export async function loadOverviewPageData(options?: {
	client?: FinApiClient;
}): Promise<OverviewPageData> {
	const client = options?.client ?? createFinApiClient();
	const shell = await loadShellState(client);
	const base = createEmptyOverview(shell);

	if (!shell.config) {
		return base;
	}

	const [accountsResult, totalBalanceResult, projectionResult] = await Promise.all([
		safeFetch(() => client.viewAccounts()),
		safeFetch(() => client.dashboardBalances({ limit: SERIES_LIMIT })),
		safeFetch(() =>
			client.dashboardProjection({
				consolidated: true,
				include: shell.availableGroups.length > 0 ? shell.availableGroups.join(",") : undefined,
				months: PROJECTION_MONTHS,
			}),
		),
	]);

	const chartAccounts = selectChartAccounts(shell, accountsResult);
	const downsampleMinStepDays = deriveDownsampleStepDays(totalBalanceResult);
	const accountBalanceSeries = await loadAccountBalanceSeries(client, chartAccounts, downsampleMinStepDays);
	const totalBalanceSeries = mapBalanceSeries(totalBalanceResult);
	const visibleAccountIds = new Set(
		chartAccounts
			.filter((account) => account.latestBalanceMinor !== null || (accountBalanceSeries[account.id]?.length ?? 0) > 0)
			.map((account) => account.id),
	);
	const visibleAccounts = chartAccounts.filter((account) => visibleAccountIds.has(account.id));

	return {
		...base,
		chartAccounts: visibleAccounts,
		totalBalanceSeries,
		accountBalanceSeries: Object.fromEntries(
			Object.entries(accountBalanceSeries).filter(([accountId, series]) => {
				return visibleAccountIds.has(accountId) && series.length > 0;
			}),
		),
		projection: mapProjection(projectionResult),
	};
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
	chartAccounts: OverviewChartAccount[],
	downsampleMinStepDays: number | undefined,
): Promise<Record<string, BalancePoint[]>> {
	if (chartAccounts.length === 0) {
		return {};
	}

	const settled = await Promise.allSettled(
		chartAccounts.map(async (account) => {
			const payload = await client.dashboardBalances({
				account: account.id,
				limit: SERIES_LIMIT,
				...(downsampleMinStepDays ? { downsampleMinStepDays } : {}),
			});
			return [account.id, mapBalanceSeries(payload)] as const;
		}),
	);

	const seriesEntries: Array<[string, BalancePoint[]]> = [];
	for (const result of settled) {
		if (result.status === "fulfilled") {
			seriesEntries.push([result.value[0], result.value[1]]);
		}
	}

	for (const account of chartAccounts) {
		if (!seriesEntries.some(([accountId]) => accountId === account.id)) {
			seriesEntries.push([account.id, []]);
		}
	}

	return Object.fromEntries(seriesEntries);
}

function createEmptyOverview(shell: ShellState): OverviewPageData {
	return {
		availableGroups: shell.availableGroups,
		groupMetadata: shell.groupMetadata,
		connection: shell.connection,
		chartAccounts: [],
		totalBalanceSeries: [],
		accountBalanceSeries: {},
		projection: null,
		totalSeriesId: TOTAL_SERIES_ID,
	};
}

function selectChartAccounts(shell: ShellState, accountsResult: ViewAccountsData | null): OverviewChartAccount[] {
	const accountRows = accountsResult?.accounts ?? [];
	const assetRows = accountRows.filter((account) => account.account_type === "asset");
	const assetRowIds = new Set(assetRows.map((account) => account.id));
	const rowById = new Map(assetRows.map((account) => [account.id, account] as const));
	const orderedConfigured = flattenConfiguredAccounts(shell.config, shell.availableGroups);
    const selected = new Map<string, OverviewChartAccount>();
    const groupOrder = new Map(shell.availableGroups.map((groupId, index) => [groupId, index]));

	for (const account of orderedConfigured) {
		if (assetRowIds.size > 0 && !assetRowIds.has(account.id) && !account.id.startsWith("Assets:")) {
			continue;
		}
		selected.set(account.id, buildOverviewChartAccount(account, rowById.get(account.id)));
	}

	for (const row of assetRows) {
		if (selected.has(row.id)) {
			continue;
		}

		const groupId = inferGroupId(row.id, shell.availableGroups);
		selected.set(row.id, {
			id: row.id,
			label: deriveAccountLabel(row.id, row.name),
			groupId,
			provider: "unknown",
			subtype: null,
			latestBalanceMinor: row.balance_minor,
			updatedAt: row.updated_at,
		});
	}

    return [...selected.values()].sort((left, right) => {
        const groupDelta =
            (groupOrder.get(left.groupId) ?? Number.MAX_SAFE_INTEGER) -
            (groupOrder.get(right.groupId) ?? Number.MAX_SAFE_INTEGER);
        if (groupDelta !== 0) {
            return groupDelta;
        }
        return left.label.localeCompare(right.label) || left.id.localeCompare(right.id);
    });
}

function buildOverviewChartAccount(account: ConfiguredChartAccount, row: AccountBalanceRow | undefined): OverviewChartAccount {
	return {
		id: account.id,
		label: deriveAccountLabel(account.id, account.label, row?.name),
		groupId: account.groupId,
		provider: account.provider,
		subtype: account.subtype,
		latestBalanceMinor: row?.balance_minor ?? null,
		updatedAt: row?.updated_at ?? null,
	};
}

function deriveAccountLabel(id: string, ...candidates: Array<string | null | undefined>): string {
	for (const candidate of candidates) {
		if (candidate && candidate.trim().length > 0) {
			return candidate;
		}
	}
	return id.split(":").at(-1) ?? id;
}

function inferGroupId(accountId: string, availableGroups: readonly string[]): string {
	const segments = accountId
		.split(":")
		.map((segment) => segment.trim().toLowerCase())
		.filter(Boolean);
	for (const groupId of availableGroups) {
		if (segments.includes(groupId.toLowerCase())) {
			return groupId;
		}
	}
	return availableGroups[0] ?? "personal";
}

type ConfiguredChartAccount = {
	id: string;
	label: string | null;
	groupId: string;
	provider: string;
	subtype: string | null;
};

function flattenConfiguredAccounts(
	config: ConfigShowData | null,
	availableGroups: readonly string[],
): ConfiguredChartAccount[] {
	if (!config) {
		return [];
	}

    const entries: ConfiguredChartAccount[] = [];

    for (const groupId of availableGroups) {
        const accounts = config.accounts[groupId] ?? [];
        for (const account of accounts) {
            entries.push({
                id: account.id,
				label: account.label ?? null,
				groupId,
				provider: account.provider,
				subtype: account.subtype ?? null,
			});
        }
    }

    const groupOrder = new Map(availableGroups.map((groupId, index) => [groupId, index]));
    return entries.sort((left, right) => {
        const groupDelta =
            (groupOrder.get(left.groupId) ?? Number.MAX_SAFE_INTEGER) -
            (groupOrder.get(right.groupId) ?? Number.MAX_SAFE_INTEGER);
        if (groupDelta !== 0) {
            return groupDelta;
        }
        return deriveAccountLabel(left.id, left.label).localeCompare(deriveAccountLabel(right.id, right.label));
    });
}

function deriveDownsampleStepDays(payload: DashboardBalanceData | null): number | undefined {
	const series = payload?.series;
	if (!series || series.length < 2) {
		return undefined;
	}

	const first = series[0]?.date ? Date.parse(`${series[0].date}T00:00:00Z`) : Number.NaN;
	const last = series.at(-1)?.date ? Date.parse(`${series.at(-1)?.date}T00:00:00Z`) : Number.NaN;
	if (!Number.isFinite(first) || !Number.isFinite(last) || last <= first) {
		return undefined;
	}

	const spanDays = Math.ceil((last - first) / DAY_MS);
	const stepDays = chooseDownsampleStepDays(spanDays);
	return stepDays > 1 ? stepDays : undefined;
}

function mapBalanceSeries(payload: DashboardBalanceData | null): BalancePoint[] {
	if (!payload) {
		return [];
	}
	return payload.series.map((point) => ({
		date: point.date,
		balanceMinor: point.balance_minor,
	}));
}

function mapProjection(payload: DashboardProjectionData | null): OverviewProjection | null {
	if (!payload) {
		return null;
	}

	const currentBurn = payload.report.scenarios.find((scenario) => scenario.kind === "current_burn") ?? null;
	const minimumBurn = payload.report.scenarios.find((scenario) => scenario.kind === "minimum_burn") ?? null;

	return {
		groups: payload.groups,
		liquidBalanceMinor: payload.report.liquid_balance_minor,
		currentBurnMinor: payload.report.current_burn_minor,
		minimumBurnMinor: payload.report.minimum_burn_minor,
		medianMonthlyExpenseMinor: payload.report.median_monthly_expense_minor,
		thresholds: {
			warningMinor: payload.report.thresholds.warning_minor,
			thresholdMinor: payload.report.thresholds.threshold_minor,
		},
		assumptions: {
			asOfDate: payload.report.assumptions.as_of_date,
			projectionMonths: payload.report.assumptions.projection_months,
			trailingOutflowWindowMonths: payload.report.assumptions.trailing_outflow_window_months,
			burnRateMethod: payload.report.assumptions.burn_rate_method,
			minimumBurnRatio: payload.report.assumptions.minimum_burn_ratio,
			fullMonthsOnly: payload.report.assumptions.full_months_only,
			includeAsOfMonthInHistory: payload.report.assumptions.include_as_of_month_in_history,
		},
		currentBurn: mapScenario(currentBurn),
		minimumBurn: mapScenario(minimumBurn),
	};
}

function mapScenario(
	scenario: DashboardProjectionData["report"]["scenarios"][number] | null,
): OverviewProjectionScenario | null {
	if (!scenario) {
		return null;
	}

	return {
		kind: scenario.kind,
		label: scenario.label,
		burnRateMinor: scenario.burn_rate_minor,
		isNetPositive: scenario.is_net_positive,
		zeroBalanceCrossing: mapCrossing(scenario.zero_balance_crossing),
		warningCrossing: mapCrossing(scenario.warning_crossing),
		thresholdCrossing: mapCrossing(scenario.threshold_crossing),
		points: scenario.points.map((point) => ({
			month: point.month_index,
			date: point.date,
			balanceMinor: point.balance_minor,
		})),
	};
}

function mapCrossing(
	crossing: DashboardProjectionData["report"]["scenarios"][number]["zero_balance_crossing"],
): OverviewProjectionCrossing | null {
	if (!crossing) {
		return null;
	}
	return {
		monthIndex: crossing.month_index,
		date: crossing.date,
		balanceMinor: crossing.balance_minor,
	};
}
