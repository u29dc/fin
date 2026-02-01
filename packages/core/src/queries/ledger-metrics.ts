import type { Database } from 'bun:sqlite';
import { getFinanceConfig } from '../config';
import { getBurnRateExcludeAccounts, getBurnRateMethod, getGroupChartAccounts, getGroupMetadata, isConfigInitialized } from '../config/index';
import { getConsolidatedMonthlyCashflow, getLedgerAccountsDailyBalanceSeries, getLedgerMonthlyCashflowSeries, type LedgerBalanceSeriesOptions, type LedgerMonthlyCashflowPoint } from './ledger';

// ============================================
// TYPES (matching metrics.ts)
// ============================================

export type LedgerGroupId = string; // Dynamic: any group ID defined in config

export type LedgerHealthAssumptions = {
	trailingExpenseWindowMonths: number;
	expenseReserveMonths: number;
	corpTaxRate: number;
	personalIncomeTaxRate: number;
	taxYearStartMonth: number;
};

export type LedgerRunwayAssumptions = {
	trailingOutflowWindowMonths: number;
	maxRunwayMonths: number;
};

export type LedgerScenarioToggles = {
	includeDividends: boolean;
	includeSalary: boolean;
	includeJointExpenses: boolean;
};

export type LedgerScenarioConfig = {
	dividendsMonthlyMinor: number;
	salaryMonthlyMinor: number;
	jointExpensesMonthlyMinor: number;
};

export type LedgerGroupHealthPoint = {
	date: string;
	healthMinor: number;
};

export type LedgerGroupRunwayPoint = {
	date: string;
	runwayMonths: number;
	isNetPositive?: boolean;
	medianExpenseMinor?: number;
};

export type LedgerGroupReserveBreakdownPoint = {
	date: string;
	balanceMinor: number;
	taxReserveMinor: number;
	expenseReserveMinor: number;
	availableMinor: number;
};

// ============================================
// DYNAMIC ACCESSORS
// ============================================

function getGroupAccountIds(groupId: LedgerGroupId): string[] {
	if (!isConfigInitialized()) {
		return [];
	}
	return getGroupChartAccounts()[groupId] ?? [];
}

function getDefaultScenarioConfig(): LedgerScenarioConfig {
	if (!isConfigInitialized()) {
		return {
			dividendsMonthlyMinor: 0,
			salaryMonthlyMinor: 0,
			jointExpensesMonthlyMinor: 0,
		};
	}
	const cfg = getFinanceConfig();
	return {
		dividendsMonthlyMinor: cfg.scenario.dividendsMonthlyMinor,
		salaryMonthlyMinor: cfg.scenario.salaryMonthlyMinor,
		jointExpensesMonthlyMinor: cfg.scenario.jointExpensesMonthlyMinor,
	};
}

// ============================================
// CONSTANTS
// ============================================

const DEFAULT_HEALTH_ASSUMPTIONS: LedgerHealthAssumptions = {
	trailingExpenseWindowMonths: 3,
	expenseReserveMonths: 1,
	corpTaxRate: 0.25,
	personalIncomeTaxRate: 0,
	taxYearStartMonth: 1,
};

const DEFAULT_RUNWAY_ASSUMPTIONS: LedgerRunwayAssumptions = {
	trailingOutflowWindowMonths: 6,
	maxRunwayMonths: 120,
};

// ============================================
// UTILITY FUNCTIONS
// ============================================

export function getLedgerGroupChartAccountIds(groupId: LedgerGroupId): string[] {
	return getGroupAccountIds(groupId);
}

function clampInt(value: number, min: number, max: number): number {
	if (!Number.isFinite(value)) {
		return min;
	}
	return Math.min(max, Math.max(min, Math.trunc(value)));
}

function median(values: number[]): number {
	if (values.length === 0) {
		return 0;
	}
	const sorted = [...values].sort((a, b) => a - b);
	const mid = Math.floor(sorted.length / 2);
	if (sorted.length % 2 === 1) {
		return sorted[mid] ?? 0;
	}
	const a = sorted[mid - 1];
	const b = sorted[mid];
	if (typeof a !== 'number' || typeof b !== 'number') {
		return 0;
	}
	return Math.round((a + b) / 2);
}

function monthToYearMonth(month: string): { year: number; month: number } {
	const year = Number(month.slice(0, 4));
	const monthNum = Number(month.slice(5, 7));
	return { year, month: monthNum };
}

function getTaxYearKey(month: string, taxYearStartMonth: number): string {
	const { year, month: monthNum } = monthToYearMonth(month);
	const taxYearStartYear = monthNum >= taxYearStartMonth ? year : year - 1;
	const startMonth = String(taxYearStartMonth).padStart(2, '0');
	return `${taxYearStartYear}-${startMonth}`;
}

function getTaxRateForGroup(assumptions: LedgerHealthAssumptions, groupId: LedgerGroupId): number {
	const meta = getGroupMetadata(groupId);
	if (meta.taxType === 'corp') {
		return assumptions.corpTaxRate;
	}
	if (meta.taxType === 'income') {
		return assumptions.personalIncomeTaxRate;
	}
	return 0;
}

// ============================================
// SCENARIO HELPERS
// ============================================

export type LedgerScenarioCashflowDelta = {
	incomeMinorDelta: number;
	expenseMinorDelta: number;
};

export function getLedgerScenarioCashflowDelta(groupId: LedgerGroupId, toggles: LedgerScenarioToggles, config: LedgerScenarioConfig): LedgerScenarioCashflowDelta {
	if (groupId === 'business') {
		return {
			incomeMinorDelta: 0,
			expenseMinorDelta: (toggles.includeDividends ? config.dividendsMonthlyMinor : 0) + (toggles.includeSalary ? config.salaryMonthlyMinor : 0),
		};
	}

	if (groupId === 'personal') {
		return {
			incomeMinorDelta: (toggles.includeDividends ? config.dividendsMonthlyMinor : 0) + (toggles.includeSalary ? config.salaryMonthlyMinor : 0),
			expenseMinorDelta: toggles.includeJointExpenses ? config.jointExpensesMonthlyMinor : 0,
		};
	}

	return { incomeMinorDelta: 0, expenseMinorDelta: 0 };
}

function getLedgerScenarioTaxNetDelta(groupId: LedgerGroupId, toggles: LedgerScenarioToggles, config: LedgerScenarioConfig): number {
	if (groupId === 'business') {
		return toggles.includeSalary ? -config.salaryMonthlyMinor : 0;
	}

	if (groupId === 'personal') {
		return (
			(toggles.includeDividends ? config.dividendsMonthlyMinor : 0) +
			(toggles.includeSalary ? config.salaryMonthlyMinor : 0) -
			(toggles.includeJointExpenses ? config.jointExpensesMonthlyMinor : 0)
		);
	}

	return 0;
}

// ============================================
// RESERVE CALCULATION
// ============================================

type MonthlyReserves = {
	expenseReserveMinor: number;
	taxReserveMinor: number;
};

function buildMonthlyReserves(monthlyCashflow: LedgerMonthlyCashflowPoint[], assumptions: LedgerHealthAssumptions, groupId: LedgerGroupId): Map<string, MonthlyReserves> {
	const trailingMonths = clampInt(assumptions.trailingExpenseWindowMonths, 1, 120);
	const expenseReserveMonths = Math.max(0, assumptions.expenseReserveMonths);
	const taxRate = Math.max(0, getTaxRateForGroup(assumptions, groupId));
	const taxYearStartMonth = clampInt(assumptions.taxYearStartMonth, 1, 12);

	const reservesByMonth = new Map<string, MonthlyReserves>();

	let currentTaxYearKey: string | null = null;
	let ytdNetMinor = 0;

	for (let i = 0; i < monthlyCashflow.length; i++) {
		const row = monthlyCashflow[i];
		if (!row) {
			continue;
		}

		const taxYearKey = getTaxYearKey(row.month, taxYearStartMonth);
		if (taxYearKey !== currentTaxYearKey) {
			currentTaxYearKey = taxYearKey;
			ytdNetMinor = 0;
		}

		ytdNetMinor += row.netMinor;

		let expenseSum = 0;
		let expenseCount = 0;
		const start = Math.max(0, i - trailingMonths + 1);
		for (let j = start; j <= i; j++) {
			const other = monthlyCashflow[j];
			if (!other) {
				continue;
			}
			expenseSum += other.expenseMinor;
			expenseCount += 1;
		}

		const avgExpenseMinor = expenseCount === 0 ? 0 : Math.round(expenseSum / expenseCount);
		const expenseReserveMinor = Math.round(avgExpenseMinor * expenseReserveMonths);

		const profitMinor = Math.max(0, ytdNetMinor);
		const taxReserveMinor = Math.round(profitMinor * taxRate);

		reservesByMonth.set(row.month, { expenseReserveMinor, taxReserveMinor });
	}

	return reservesByMonth;
}

// ============================================
// HEALTH SERIES
// ============================================

export function getLedgerGroupDailyHealthSeries(
	db: Database,
	groupId: LedgerGroupId,
	options: LedgerBalanceSeriesOptions = {},
	assumptions: Partial<LedgerHealthAssumptions> = {},
	scenario?: LedgerScenarioToggles,
	scenarioConfig: Partial<LedgerScenarioConfig> = {},
): LedgerGroupHealthPoint[] {
	const mergedAssumptions: LedgerHealthAssumptions = {
		...DEFAULT_HEALTH_ASSUMPTIONS,
		...assumptions,
	};
	const mergedScenarioConfig: LedgerScenarioConfig = {
		...getDefaultScenarioConfig(),
		...scenarioConfig,
	};

	const chartAccountIds = getLedgerGroupChartAccountIds(groupId);
	const cashSeries = getLedgerAccountsDailyBalanceSeries(db, chartAccountIds, options);
	if (cashSeries.length === 0) {
		return [];
	}

	const cashflowOptions: { from?: string; to?: string; limit?: number } = {
		limit: 1_200,
	};
	if (options.from) {
		cashflowOptions.from = options.from;
	}
	if (options.to) {
		cashflowOptions.to = options.to;
	}

	const monthlyCashflow = getLedgerMonthlyCashflowSeries(db, chartAccountIds, cashflowOptions);

	// Apply scenario adjustments for reserve calculations
	const cashflowForReserves: LedgerMonthlyCashflowPoint[] =
		scenario && (scenario.includeDividends || scenario.includeSalary || scenario.includeJointExpenses)
			? (() => {
					const expenseDelta = getLedgerScenarioCashflowDelta(groupId, scenario, mergedScenarioConfig).expenseMinorDelta;
					const taxNetDelta = getLedgerScenarioTaxNetDelta(groupId, scenario, mergedScenarioConfig);

					if (expenseDelta === 0 && taxNetDelta === 0) {
						return monthlyCashflow;
					}

					return monthlyCashflow.map((row) => ({
						...row,
						expenseMinor: row.expenseMinor + expenseDelta,
						netMinor: row.netMinor + taxNetDelta,
					}));
				})()
			: monthlyCashflow;

	const monthlyReserves = buildMonthlyReserves(cashflowForReserves, mergedAssumptions, groupId);
	const months = Array.from(monthlyReserves.keys()).sort();

	let monthIndex = -1;
	let currentReserves: MonthlyReserves = { expenseReserveMinor: 0, taxReserveMinor: 0 };

	const health: LedgerGroupHealthPoint[] = [];

	for (const point of cashSeries) {
		const month = point.date.slice(0, 7);

		while (monthIndex + 1 < months.length) {
			const next = months[monthIndex + 1];
			if (!next || next > month) {
				break;
			}
			monthIndex += 1;
			currentReserves = monthlyReserves.get(next) ?? currentReserves;
		}

		const healthMinor = point.balanceMinor - currentReserves.expenseReserveMinor - currentReserves.taxReserveMinor;
		health.push({ date: point.date, healthMinor });
	}

	return health;
}

// ============================================
// RUNWAY SERIES
// ============================================

function getRunwayBalanceChartAccountIds(groupId: LedgerGroupId): string[] {
	const groupAccounts = getLedgerGroupChartAccountIds(groupId);
	if (groupId === 'personal') {
		// Exclude investment accounts (Vanguard) - not liquid cash for runway
		return groupAccounts.filter((id) => !id.toLowerCase().includes('vanguard'));
	}
	return groupAccounts;
}

function getRunwayExpenseChartAccountIds(groupId: LedgerGroupId): string[] {
	const groupAccounts = getLedgerGroupChartAccountIds(groupId);
	if (groupId === 'business') {
		// Exclude Wise for expenses - has VAT/tax payments that are irrelevant
		// when calculating runway (no revenue = no new VAT/tax)
		return groupAccounts.filter((id) => !id.toLowerCase().includes('wise'));
	}
	if (groupId === 'personal') {
		// Exclude investment accounts (Vanguard) - not liquid cash
		return groupAccounts.filter((id) => !id.toLowerCase().includes('vanguard'));
	}
	return groupAccounts;
}

type MonthlyOutflowPoint = {
	month: string;
	outflowMinor: number;
};

function getMonthlyTrailingAverageOutflowMap(monthlyOutflow: MonthlyOutflowPoint[], trailingMonths: number): Map<string, number> {
	const map = new Map<string, number>();

	for (let i = 0; i < monthlyOutflow.length; i++) {
		let sum = 0;
		let count = 0;
		const start = Math.max(0, i - trailingMonths + 1);
		for (let j = start; j <= i; j++) {
			const other = monthlyOutflow[j];
			if (!other) {
				continue;
			}
			sum += other.outflowMinor;
			count += 1;
		}

		const avg = count === 0 ? 0 : Math.round(sum / count);
		const month = monthlyOutflow[i]?.month;
		if (month) {
			map.set(month, avg);
		}
	}

	return map;
}

function getMonthlyTrailingMedianOutflowMap(monthlyOutflow: MonthlyOutflowPoint[], trailingMonths: number): Map<string, number> {
	const map = new Map<string, number>();

	for (let i = 0; i < monthlyOutflow.length; i++) {
		const start = Math.max(0, i - trailingMonths + 1);
		const values: number[] = [];
		for (let j = start; j <= i; j++) {
			const other = monthlyOutflow[j];
			if (other) {
				values.push(other.outflowMinor);
			}
		}

		const med = median(values);
		const month = monthlyOutflow[i]?.month;
		if (month) {
			map.set(month, med);
		}
	}

	return map;
}

function getMonthlyTrailingOutflowMap(monthlyOutflow: MonthlyOutflowPoint[], trailingMonths: number, method: 'mean' | 'median'): Map<string, number> {
	if (method === 'median') {
		return getMonthlyTrailingMedianOutflowMap(monthlyOutflow, trailingMonths);
	}
	return getMonthlyTrailingAverageOutflowMap(monthlyOutflow, trailingMonths);
}

function clampNumber(value: number, min: number, max: number): number {
	if (!Number.isFinite(value)) {
		return min;
	}
	return Math.min(max, Math.max(min, value));
}

function computeScenarioOutflowDelta(groupId: LedgerGroupId, scenario: LedgerScenarioToggles | undefined, scenarioConfig: LedgerScenarioConfig): number {
	if (!scenario) return 0;
	const hasScenarioToggle = scenario.includeDividends || scenario.includeSalary || scenario.includeJointExpenses;
	if (!hasScenarioToggle) return 0;
	return getLedgerScenarioCashflowDelta(groupId, scenario, scenarioConfig).expenseMinorDelta;
}

function computeRunwayPoint(date: string, balanceMinor: number, avgOutflowMinor: number, maxRunwayMonths: number, medianExpenseMinor: number | undefined): LedgerGroupRunwayPoint {
	if (avgOutflowMinor <= 0) {
		const runwayMonths = balanceMinor > 0 ? maxRunwayMonths : 0;
		return {
			date,
			runwayMonths,
			isNetPositive: true,
			...(medianExpenseMinor !== undefined && { medianExpenseMinor }),
		};
	}
	const rawRunway = balanceMinor / avgOutflowMinor;
	const clamped = clampNumber(rawRunway, 0, maxRunwayMonths);
	const runwayMonths = Math.round(clamped * 100) / 100;
	return {
		date,
		runwayMonths,
		...(medianExpenseMinor !== undefined && { medianExpenseMinor }),
	};
}

export function getLedgerGroupDailyRunwaySeries(
	db: Database,
	groupId: LedgerGroupId,
	options: LedgerBalanceSeriesOptions = {},
	assumptions: Partial<LedgerRunwayAssumptions> = {},
	scenario?: LedgerScenarioToggles,
	scenarioConfig: Partial<LedgerScenarioConfig> = {},
): LedgerGroupRunwayPoint[] {
	const mergedAssumptions: LedgerRunwayAssumptions = { ...DEFAULT_RUNWAY_ASSUMPTIONS, ...assumptions };
	const mergedScenarioConfig: LedgerScenarioConfig = { ...getDefaultScenarioConfig(), ...scenarioConfig };

	const trailingMonths = clampInt(mergedAssumptions.trailingOutflowWindowMonths, 1, 120);
	const maxRunwayMonths = clampInt(mergedAssumptions.maxRunwayMonths, 1, 600);

	const balanceChartAccountIds = getRunwayBalanceChartAccountIds(groupId);
	const expenseChartAccountIds = getRunwayExpenseChartAccountIds(groupId);

	const cashSeries = getLedgerAccountsDailyBalanceSeries(db, balanceChartAccountIds, options);
	if (cashSeries.length === 0) return [];

	const cashflowOptions: { limit: number; from?: string; to?: string } = { limit: 200_000 };
	if (options.from) cashflowOptions.from = options.from;
	if (options.to) cashflowOptions.to = options.to;
	const baseMonthlyCashflow = getLedgerMonthlyCashflowSeries(db, expenseChartAccountIds, cashflowOptions);
	const outflowDelta = computeScenarioOutflowDelta(groupId, scenario, mergedScenarioConfig);

	const burnMethod = getBurnRateMethod();
	const monthlyOutflow: MonthlyOutflowPoint[] = baseMonthlyCashflow.map((row) => ({
		month: row.month,
		outflowMinor: row.expenseMinor + outflowDelta,
	}));
	const outflowByMonth = getMonthlyTrailingOutflowMap(monthlyOutflow, trailingMonths, burnMethod);
	const months = Array.from(outflowByMonth.keys()).sort();

	// Median uses BASE expenses only (no salary/dividends) - shows operating costs
	const last12Base = baseMonthlyCashflow.slice(-13, -1);
	const medianExpenseMinor = median(last12Base.map((p) => p.expenseMinor));

	let monthIndex = -1;
	let currentOutflowMinor: number | null = null;
	const runway: LedgerGroupRunwayPoint[] = [];

	for (const point of cashSeries) {
		const month = point.date.slice(0, 7);

		while (monthIndex + 1 < months.length) {
			const nextMonth = months[monthIndex + 1];
			if (!nextMonth || nextMonth > month) break;
			monthIndex += 1;
			currentOutflowMinor = outflowByMonth.get(nextMonth) ?? currentOutflowMinor;
		}

		runway.push(computeRunwayPoint(point.date, point.balanceMinor, currentOutflowMinor ?? 0, maxRunwayMonths, medianExpenseMinor));
	}

	return runway;
}

// ============================================
// CONSOLIDATED RUNWAY SERIES
// ============================================

export type ConsolidatedRunwayOptions = {
	includeGroups: string[];
	from?: string;
	to?: string;
	limit?: number;
};

export type ConsolidatedRunwayPoint = {
	date: string;
	balanceMinor: number;
	burnRateMinor: number;
	runwayMonths: number;
	isNetPositive?: boolean;
};

function collectLiquidChartAccountIds(includeGroups: string[]): string[] {
	const accountIds: string[] = [];
	for (const groupId of includeGroups) {
		const groupAccounts = getLedgerGroupChartAccountIds(groupId);
		const liquidAccounts = groupAccounts.filter((id) => !id.toLowerCase().includes('vanguard'));
		accountIds.push(...liquidAccounts);
	}
	return accountIds;
}

function computeConsolidatedRunwayPoint(date: string, balanceMinor: number, avgOutflow: number, maxRunwayMonths: number): ConsolidatedRunwayPoint {
	if (avgOutflow <= 0) {
		const runwayMonths = balanceMinor > 0 ? maxRunwayMonths : 0;
		return { date, balanceMinor, burnRateMinor: 0, runwayMonths, isNetPositive: true };
	}
	const rawRunway = balanceMinor / avgOutflow;
	const clamped = clampNumber(rawRunway, 0, maxRunwayMonths);
	const runwayMonths = Math.round(clamped * 100) / 100;
	return { date, balanceMinor, burnRateMinor: avgOutflow, runwayMonths };
}

/**
 * Get consolidated runway across multiple groups, excluding internal transfers.
 * This answers "how long until I'm broke across ALL accounts I control."
 *
 * Internal transfers (money moving between accounts in included groups) are excluded
 * from burn calculation because they don't represent money leaving the system.
 */
export function getLedgerConsolidatedDailyRunwaySeries(db: Database, options: ConsolidatedRunwayOptions, assumptions: Partial<LedgerRunwayAssumptions> = {}): ConsolidatedRunwayPoint[] {
	const { includeGroups, from, to, limit = 10_000 } = options;
	if (includeGroups.length === 0) return [];

	const mergedAssumptions: LedgerRunwayAssumptions = { ...DEFAULT_RUNWAY_ASSUMPTIONS, ...assumptions };
	const trailingMonths = clampInt(mergedAssumptions.trailingOutflowWindowMonths, 1, 120);
	const maxRunwayMonths = clampInt(mergedAssumptions.maxRunwayMonths, 1, 600);

	const allChartAccountIds = collectLiquidChartAccountIds(includeGroups);
	if (allChartAccountIds.length === 0) return [];

	const balanceOptions: LedgerBalanceSeriesOptions = { limit };
	if (from) balanceOptions.from = from;
	if (to) balanceOptions.to = to;

	const cashSeries = getLedgerAccountsDailyBalanceSeries(db, allChartAccountIds, balanceOptions);
	if (cashSeries.length === 0) return [];

	const cashflowOptions: { limit: number; from?: string; to?: string } = { limit: 200_000 };
	if (from) cashflowOptions.from = from;
	if (to) cashflowOptions.to = to;

	const burnMethod = getBurnRateMethod();
	const excludePrefixes = getBurnRateExcludeAccounts();
	const monthlyCashflow = getConsolidatedMonthlyCashflow(db, allChartAccountIds, cashflowOptions, excludePrefixes);
	const monthlyOutflow: MonthlyOutflowPoint[] = monthlyCashflow.map((row) => ({ month: row.month, outflowMinor: row.expenseMinor }));
	const outflowByMonth = getMonthlyTrailingOutflowMap(monthlyOutflow, trailingMonths, burnMethod);
	const months = Array.from(outflowByMonth.keys()).sort();

	let monthIndex = -1;
	let currentOutflowMinor: number | null = null;
	const runway: ConsolidatedRunwayPoint[] = [];

	for (const point of cashSeries) {
		const month = point.date.slice(0, 7);
		let nextMonth = months[monthIndex + 1];
		while (nextMonth !== undefined && nextMonth <= month) {
			monthIndex += 1;
			currentOutflowMinor = outflowByMonth.get(nextMonth) ?? currentOutflowMinor;
			nextMonth = months[monthIndex + 1];
		}
		runway.push(computeConsolidatedRunwayPoint(point.date, point.balanceMinor, currentOutflowMinor ?? 0, maxRunwayMonths));
	}

	return runway;
}

// ============================================
// RESERVE BREAKDOWN SERIES
// ============================================

export function getLedgerGroupDailyReserveBreakdownSeries(
	db: Database,
	groupId: LedgerGroupId,
	options: LedgerBalanceSeriesOptions = {},
	assumptions: Partial<LedgerHealthAssumptions> = {},
	scenario?: LedgerScenarioToggles,
	scenarioConfig: Partial<LedgerScenarioConfig> = {},
): LedgerGroupReserveBreakdownPoint[] {
	const mergedAssumptions: LedgerHealthAssumptions = {
		...DEFAULT_HEALTH_ASSUMPTIONS,
		...assumptions,
	};
	const mergedScenarioConfig: LedgerScenarioConfig = {
		...getDefaultScenarioConfig(),
		...scenarioConfig,
	};

	const chartAccountIds = getLedgerGroupChartAccountIds(groupId);
	const cashSeries = getLedgerAccountsDailyBalanceSeries(db, chartAccountIds, options);
	if (cashSeries.length === 0) {
		return [];
	}

	const cashflowOptions: { from?: string; to?: string; limit?: number } = {
		limit: 1_200,
	};
	if (options.from) {
		cashflowOptions.from = options.from;
	}
	if (options.to) {
		cashflowOptions.to = options.to;
	}

	const monthlyCashflow = getLedgerMonthlyCashflowSeries(db, chartAccountIds, cashflowOptions);

	// Apply scenario adjustments
	const cashflowForReserves: LedgerMonthlyCashflowPoint[] =
		scenario && (scenario.includeDividends || scenario.includeSalary || scenario.includeJointExpenses)
			? (() => {
					const expenseDelta = getLedgerScenarioCashflowDelta(groupId, scenario, mergedScenarioConfig).expenseMinorDelta;
					const taxNetDelta = getLedgerScenarioTaxNetDelta(groupId, scenario, mergedScenarioConfig);

					if (expenseDelta === 0 && taxNetDelta === 0) {
						return monthlyCashflow;
					}

					return monthlyCashflow.map((row) => ({
						...row,
						expenseMinor: row.expenseMinor + expenseDelta,
						netMinor: row.netMinor + taxNetDelta,
					}));
				})()
			: monthlyCashflow;

	const monthlyReserves = buildMonthlyReserves(cashflowForReserves, mergedAssumptions, groupId);
	const months = Array.from(monthlyReserves.keys()).sort();

	let monthIndex = -1;
	let currentReserves: MonthlyReserves = { expenseReserveMinor: 0, taxReserveMinor: 0 };

	const breakdown: LedgerGroupReserveBreakdownPoint[] = [];

	for (const point of cashSeries) {
		const month = point.date.slice(0, 7);

		while (monthIndex + 1 < months.length) {
			const next = months[monthIndex + 1];
			if (!next || next > month) {
				break;
			}
			monthIndex += 1;
			currentReserves = monthlyReserves.get(next) ?? currentReserves;
		}

		const availableMinor = Math.max(0, point.balanceMinor - currentReserves.expenseReserveMinor - currentReserves.taxReserveMinor);
		breakdown.push({
			date: point.date,
			balanceMinor: point.balanceMinor,
			taxReserveMinor: currentReserves.taxReserveMinor,
			expenseReserveMinor: currentReserves.expenseReserveMinor,
			availableMinor,
		});
	}

	return breakdown;
}
