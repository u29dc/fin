import type { Database } from 'bun:sqlite';

import type { AssetAccountId, GroupId } from '../types/chart-account-ids';
import { getGroupChartAccounts, isGroupId } from '../types/chart-account-ids';
import type { ExpenseNode } from '../types/ledger';
import {
	getCashFlowData,
	getCashFlowDataMedian,
	getExpensesByCategory,
	getGroupExpenseHierarchy,
	getGroupExpenseHierarchyMedian,
	getLedgerAccountsDailyBalanceSeries,
	getLedgerAllAccountsDailyBalanceSeries,
	getLedgerCategoryMonthlyMedian,
	getLedgerCumulativeContributionSeries,
	getLedgerDailyBalanceSeries,
	getLedgerLatestBalances,
	getLedgerMonthlyCashflowSeries,
	getPureMonthlyCashflowSeries,
	type SankeyFlowData,
} from './ledger';
import { applyScenarioToCashflowSeries, getScenarioMonthlyFlows, type ScenarioConfig, type ScenarioToggles } from './metrics';

// ============================================
// GROUP TYPES & CONSTANTS
// ============================================

export { isGroupId };
export type { GroupId } from '../types/chart-account-ids';

export function getGroupChartAccountIds(groupId: string): AssetAccountId[] {
	return getGroupChartAccounts()[groupId] ?? [];
}

// ============================================
// BALANCE TYPES & QUERIES
// ============================================

export type LatestBalance = {
	chartAccountId: AssetAccountId;
	date: string | null;
	balanceMinor: number | null;
};

export function getLatestBalances(db: Database, chartAccountIds: AssetAccountId[]): LatestBalance[] {
	if (chartAccountIds.length === 0) {
		return [];
	}

	const ledgerResults = getLedgerLatestBalances(db, chartAccountIds);

	return chartAccountIds.map((chartAccountId, i) => ({
		chartAccountId,
		date: ledgerResults[i]?.date ?? null,
		balanceMinor: ledgerResults[i]?.balanceMinor ?? null,
	}));
}

export type DailyBalancePoint = {
	date: string;
	balanceMinor: number;
};

export type BalanceSeriesOptions = {
	from?: string;
	to?: string;
	limit?: number;
};

export function getDailyBalanceSeries(db: Database, chartAccountId: AssetAccountId, options: BalanceSeriesOptions = {}): DailyBalancePoint[] {
	return getLedgerDailyBalanceSeries(db, chartAccountId, options);
}

export function getAllAccountsDailyBalanceSeries(db: Database, chartAccountIds: AssetAccountId[], options: BalanceSeriesOptions = {}): Record<string, DailyBalancePoint[]> {
	return getLedgerAllAccountsDailyBalanceSeries(db, chartAccountIds, options);
}

// ============================================
// CONTRIBUTION TYPES & QUERIES
// ============================================

export type ContributionPoint = {
	date: string;
	contributionsMinor: number;
};

export type ContributionSeriesOptions = {
	from?: string;
	to?: string;
	limit?: number;
};

export function getAccountCumulativeContributionSeries(db: Database, chartAccountId: AssetAccountId, options: ContributionSeriesOptions = {}): ContributionPoint[] {
	return getLedgerCumulativeContributionSeries(db, chartAccountId, options);
}

// ============================================
// GROUP BALANCE AGGREGATION
// ============================================

export type GroupBalancePoint = {
	date: string;
	balanceMinor: number;
};

export type AccountSeriesState = {
	points: DailyBalancePoint[];
	index: number;
	current: number | null;
};

export function collectSortedDates(seriesState: AccountSeriesState[]): string[] {
	const dates = new Set<string>();
	for (const state of seriesState) {
		for (const point of state.points) {
			dates.add(point.date);
		}
	}
	return Array.from(dates.values()).sort();
}

export function mergeBalanceSeriesByDate(seriesState: AccountSeriesState[], sortedDates: string[]): GroupBalancePoint[] {
	const merged: GroupBalancePoint[] = [];

	for (const date of sortedDates) {
		let total = 0;

		for (const state of seriesState) {
			while (state.index < state.points.length && state.points[state.index]?.date === date) {
				state.current = state.points[state.index]?.balanceMinor ?? null;
				state.index += 1;
			}

			if (state.current !== null) {
				total += state.current;
			}
		}

		merged.push({ date, balanceMinor: total });
	}

	return merged;
}

export function getAccountsDailyBalanceSeries(db: Database, chartAccountIds: AssetAccountId[], options: BalanceSeriesOptions = {}): GroupBalancePoint[] {
	if (chartAccountIds.length === 0) {
		return [];
	}

	return getLedgerAccountsDailyBalanceSeries(db, chartAccountIds, options);
}

export function getGroupDailyBalanceSeries(db: Database, groupId: GroupId, options: BalanceSeriesOptions = {}): GroupBalancePoint[] {
	return getAccountsDailyBalanceSeries(db, getGroupChartAccountIds(groupId), options);
}

// ============================================
// CASHFLOW TYPES & QUERIES
// ============================================

export type MonthlyCashflowPoint = {
	month: string;
	incomeMinor: number;
	expenseMinor: number;
	netMinor: number;
	savingsRatePct: number | null;
	rollingMedianExpenseMinor: number | null;
	expenseDeviationRatio: number | null;
};

export type CashflowSeriesOptions = {
	from?: string;
	to?: string;
	limit?: number;
	includeIncomingTransfers?: boolean;
	shiftLateMonthIncome?: number;
};

export function getMonthlyCashflowSeries(db: Database, chartAccountIds: AssetAccountId[], options: CashflowSeriesOptions = {}): MonthlyCashflowPoint[] {
	const { from, to, limit = 120 } = options;

	if (chartAccountIds.length === 0) {
		return [];
	}

	const ledgerOptions: { from?: string; to?: string; limit?: number } = { limit };
	if (from) ledgerOptions.from = from;
	if (to) ledgerOptions.to = to;
	return getLedgerMonthlyCashflowSeries(db, chartAccountIds, ledgerOptions);
}

export function getGroupMonthlyCashflowSeries(db: Database, groupId: GroupId, options: CashflowSeriesOptions = {}): MonthlyCashflowPoint[] {
	return getMonthlyCashflowSeries(db, getGroupChartAccountIds(groupId), options);
}

export function getGroupMonthlyCashflowSeriesWithScenario(
	db: Database,
	groupId: GroupId,
	options: CashflowSeriesOptions = {},
	scenario: ScenarioToggles,
	scenarioConfig: Partial<ScenarioConfig> = {},
): MonthlyCashflowPoint[] {
	const base = getGroupMonthlyCashflowSeries(db, groupId, options);
	if (!scenario.includeDividends && !scenario.includeSalary && !scenario.includeJointExpenses) {
		return base;
	}

	const flows = getScenarioMonthlyFlows(db, scenarioConfig);
	return applyScenarioToCashflowSeries(base, groupId, scenario, flows);
}

/**
 * Get "pure" monthly cashflow by querying Expense/Income accounts directly.
 * This excludes internal transfers, dividend payments, round-ups, and investment transfers.
 * Shows only money that actually left/entered the system as real expenses/income.
 */
export function getGroupPureMonthlyCashflowSeries(db: Database, groupId: GroupId, options: CashflowSeriesOptions = {}): MonthlyCashflowPoint[] {
	const { from, to, limit = 120 } = options;
	const chartAccountIds = getGroupChartAccountIds(groupId);

	if (chartAccountIds.length === 0) {
		return [];
	}

	const ledgerOptions: { from?: string; to?: string; limit?: number } = { limit };
	if (from) ledgerOptions.from = from;
	if (to) ledgerOptions.to = to;
	return getPureMonthlyCashflowSeries(db, chartAccountIds, ledgerOptions);
}

// ============================================
// CATEGORY BREAKDOWN QUERIES
// ============================================

export type CategoryBreakdownPoint = {
	category: string;
	totalMinor: number;
	transactionCount: number;
};

export type CategoryBreakdownOptions = {
	months?: number;
	limit?: number;
};

export function getGroupCategoryBreakdown(db: Database, _groupId: GroupId, options: CategoryBreakdownOptions = {}): CategoryBreakdownPoint[] {
	const { months = 3, limit = 10 } = options;

	const results = getExpensesByCategory(db, months);

	return results.slice(0, limit).map((r) => ({
		category: r.categoryName,
		totalMinor: r.totalMinor,
		transactionCount: r.transactionCount,
	}));
}

// ============================================
// CATEGORY MONTHLY MEDIAN QUERIES
// ============================================

export type CategoryMonthlyMedianPoint = {
	category: string;
	monthlyMedianMinor: number;
	monthCount: number;
};

export type CategoryMonthlyMedianOptions = {
	months?: number;
	limit?: number;
};

export function getGroupCategoryMonthlyMedian(db: Database, groupId: GroupId, options: CategoryMonthlyMedianOptions = {}): CategoryMonthlyMedianPoint[] {
	const { months = 6, limit = 10 } = options;
	const chartAccountIds = getGroupChartAccountIds(groupId);

	if (chartAccountIds.length === 0) {
		return [];
	}

	const ledgerResults = getLedgerCategoryMonthlyMedian(db, chartAccountIds, { months, limit });

	return ledgerResults.map((r) => ({
		category: r.categoryName,
		monthlyMedianMinor: r.monthlyMedianMinor,
		monthCount: r.monthCount,
	}));
}

// ============================================
// GROUP EXPENSE HIERARCHY (FOR TREEMAP)
// ============================================

export type ExpenseHierarchyOptions = {
	months?: number;
};

export function getGroupExpenseTree(db: Database, groupId: GroupId, options: ExpenseHierarchyOptions = {}): ExpenseNode[] {
	const chartAccountIds = getGroupChartAccountIds(groupId);
	return getGroupExpenseHierarchy(db, chartAccountIds, options);
}

export function getGroupExpenseTreeMedian(db: Database, groupId: GroupId, options: ExpenseHierarchyOptions = {}): ExpenseNode[] {
	const chartAccountIds = getGroupChartAccountIds(groupId);
	return getGroupExpenseHierarchyMedian(db, chartAccountIds, options);
}

// ============================================
// GROUP CASH FLOW DATA (FOR SANKEY)
// ============================================

export type CashFlowDataOptions = {
	months?: number;
};

export function getGroupCashFlowData(db: Database, groupId: GroupId, options: CashFlowDataOptions = {}): SankeyFlowData {
	const chartAccountIds = getGroupChartAccountIds(groupId);
	return getCashFlowData(db, chartAccountIds, options);
}

export function getGroupCashFlowDataMedian(db: Database, groupId: GroupId, options: CashFlowDataOptions = {}): SankeyFlowData {
	const chartAccountIds = getGroupChartAccountIds(groupId);
	return getCashFlowDataMedian(db, chartAccountIds, options);
}
