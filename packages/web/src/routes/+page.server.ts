import {
	type AssetAccountId,
	type ContributionPoint,
	type ExpenseNode,
	type GroupReserveBreakdownPoint,
	type GroupRunwayPoint,
	getAccountCumulativeContributionSeries,
	getAllAccountsDailyBalanceSeries,
	getAssetAccounts,
	getFinanceConfig,
	getGroupCashFlowDataMedian,
	getGroupDailyReserveBreakdownSeries,
	getGroupDailyRunwaySeries,
	getGroupExpenseTreeMedian,
	getGroupMonthlyCashflowSeriesWithScenarioBatch,
	getLatestBalances,
	type MonthlyCashflowPoint,
	type SankeyFlowData,
} from '@fin/core';
import { getAccountById, getAllGroupMetadata, getGroupChartAccounts, getGroupIds, getGroupMetadata } from '@fin/core/config';

import { chooseDownsampleStepDays, downsampleByMinStep, toUtcMsFromIsoDate } from '$lib/charts/utils';

import { db } from '$lib/server/db';

const DAY_MS = 86_400_000;

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

function downsampleDailySeriesRecord<T extends { date: string }>(seriesById: Record<string, T[]>): Record<string, T[]> {
	return Object.fromEntries(Object.entries(seriesById).map(([id, series]) => [id, downsampleDailySeries(series)]));
}

export function load({ url }: { url: URL }) {
	const groupParam = url.searchParams.get('group');
	const financeConfig = getFinanceConfig();
	const groupChartAccounts = getGroupChartAccounts();
	const groupIds = getGroupIds();

	// Get accounts with latest balances
	const accounts = getAssetAccounts(db);
	const latestBalances = getLatestBalances(
		db,
		accounts.map((a) => a.id as AssetAccountId),
	);
	const latestById = new Map(latestBalances.map((b) => [b.chartAccountId, b]));

	const accountsWithBalances = accounts.map((account) => {
		const configAccount = getAccountById(account.id);
		return {
			...account,
			provider: configAccount?.provider ?? 'unknown',
			subtype: configAccount?.subtype,
			latestBalance: latestById.get(account.id as AssetAccountId) ?? {
				chartAccountId: account.id,
				date: null,
				balanceMinor: null,
			},
		};
	});

	// Get all chart account IDs for balance series
	const allChartAccountIds = Array.from(new Set(groupIds.flatMap((id) => groupChartAccounts[id] ?? []))) as AssetAccountId[];

	// Fetch all balance series in a single batched query
	const accountBalanceSeries = downsampleDailySeriesRecord(getAllAccountsDailyBalanceSeries(db, allChartAccountIds, { limit: 10_000 }));

	// Fetch contribution series for investment accounts (Vanguard provider)
	const accountContributionSeries: Record<string, ContributionPoint[]> = {};
	for (const accountId of allChartAccountIds) {
		const account = getAccountById(accountId);
		if (account?.provider === 'vanguard') {
			const series = getAccountCumulativeContributionSeries(db, accountId as AssetAccountId, { limit: 10_000 });
			accountContributionSeries[accountId] = downsampleDailySeries(series);
		}
	}

	// Fetch cashflow series for all groups
	const groupCashflowSeries: Record<string, MonthlyCashflowPoint[]> = getGroupMonthlyCashflowSeriesWithScenarioBatch(
		db,
		groupIds,
		{ limit: 240 },
		financeConfig.scenarioToggles,
		financeConfig.scenario,
	);

	// Fetch runway for all groups (just the latest point)
	const groupRunway: Record<string, GroupRunwayPoint | null> = Object.fromEntries(groupIds.map((id) => [id, null]));

	for (const groupId of groupIds) {
		const series = getGroupDailyRunwaySeries(db, groupId, {}, { trailingOutflowWindowMonths: financeConfig.trailingExpenseWindowMonths }, financeConfig.scenarioToggles, financeConfig.scenario);
		groupRunway[groupId] = series[series.length - 1] ?? null;
	}

	// Fetch reserve breakdown for all groups using config-driven reserve months
	const groupReserveBreakdown: Record<string, GroupReserveBreakdownPoint[]> = Object.fromEntries(groupIds.map((id) => [id, []]));

	for (const groupId of groupIds) {
		const meta = getGroupMetadata(groupId);
		const expenseReserveMonths = meta.expenseReserveMonths;
		const series = getGroupDailyReserveBreakdownSeries(db, groupId, { limit: 10_000 }, { expenseReserveMonths }, financeConfig.scenarioToggles, financeConfig.scenario);
		groupReserveBreakdown[groupId] = downsampleDailySeries(series);
	}

	// Fetch cash flow data for Sankey charts (6-month average)
	const groupCashFlowData: Record<string, SankeyFlowData> = Object.fromEntries(groupIds.map((id) => [id, { nodes: [], links: [] }]));

	for (const groupId of groupIds) {
		groupCashFlowData[groupId] = getGroupCashFlowDataMedian(db, groupId, { months: 6 });
	}

	// Fetch expense hierarchy for Treemap charts (6-month average)
	const groupExpenseHierarchy: Record<string, ExpenseNode[]> = Object.fromEntries(groupIds.map((id) => [id, []]));

	for (const groupId of groupIds) {
		groupExpenseHierarchy[groupId] = getGroupExpenseTreeMedian(db, groupId, { months: 6 });
	}

	// Build UI config from TOML config using group metadata
	type GroupConfig = { label: string; accountIds: string[]; icon: string };
	type AccountGroupConfig = { label: string; accounts: { id: string; label: string }[] };

	const groupMetadataList = getAllGroupMetadata();
	const groupMetadataMap = Object.fromEntries(groupMetadataList.map((m) => [m.id, m]));

	const uiConfig = {
		groups: groupIds.reduce<Record<string, GroupConfig>>((acc, groupId) => {
			const meta = groupMetadataMap[groupId];
			acc[groupId] = {
				label: meta?.label ?? groupId.charAt(0).toUpperCase() + groupId.slice(1),
				accountIds: groupChartAccounts[groupId] ?? [],
				icon: meta?.icon ?? 'wallet',
			};
			return acc;
		}, {}),
		accountGroupConfig: groupIds.reduce<Record<string, AccountGroupConfig>>((acc, groupId) => {
			const meta = groupMetadataMap[groupId];
			const groupAccountIds = groupChartAccounts[groupId] ?? [];
			acc[groupId] = {
				label: `${meta?.label ?? groupId.charAt(0).toUpperCase() + groupId.slice(1)} Accounts`,
				accounts: groupAccountIds.map((id) => {
					const account = getAccountById(id);
					return { id, label: account?.label ?? id.split(':').pop() ?? id };
				}),
			};
			return acc;
		}, {}),
		groupColumnOrder: groupIds,
		groupMetadata: groupMetadataMap,
	};

	// Validate group param - must be a valid group ID
	const validatedGroup = groupParam && groupIds.includes(groupParam) ? groupParam : null;

	return {
		config: { finance: financeConfig, ui: uiConfig },
		accounts: accountsWithBalances,
		accountBalanceSeries,
		accountContributionSeries,
		groupCashflowSeries,
		groupRunway,
		groupReserveBreakdown,
		groupCashFlowData,
		groupExpenseHierarchy,
		initialGroup: validatedGroup,
	};
}
