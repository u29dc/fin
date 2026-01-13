import type { Database } from 'bun:sqlite';

import { getAccountIdsByGroup, getFirstAccountIdByGroup } from '../config';
import type { AssetAccountId } from '../types/chart-account-ids';
import type { BalanceSeriesOptions, GroupId, MonthlyCashflowPoint } from './groups';
import {
	type ConsolidatedRunwayOptions,
	type ConsolidatedRunwayPoint,
	getLedgerConsolidatedDailyRunwaySeries,
	getLedgerGroupDailyHealthSeries,
	getLedgerGroupDailyReserveBreakdownSeries,
	getLedgerGroupDailyRunwaySeries,
	type LedgerGroupId,
	type LedgerScenarioConfig,
} from './ledger-metrics';

// ============================================
// SHARED UTILITIES
// ============================================

// ============================================
// SCENARIO (from scenario.ts)
// ============================================

export type ScenarioToggles = {
	includeDividends: boolean;
	includeSalary: boolean;
	includeJointExpenses: boolean;
};

export type ScenarioConfig = {
	lookbackMonths: number;
	salaryDividendSplitMinor: number;
	dividendsMonthlyMinor: number;
	salaryMonthlyMinor: number;
	jointExpensesMonthlyMinor: number;
};

export type ScenarioMonthlyFlows = {
	dividendsMonthlyMinor: number;
	salaryMonthlyMinor: number;
	jointExpensesMonthlyMinor: number;
};

export type ScenarioCashflowDelta = {
	incomeMinorDelta: number;
	expenseMinorDelta: number;
};

const DEFAULT_SCENARIO_CONFIG: ScenarioConfig = {
	lookbackMonths: 12,
	salaryDividendSplitMinor: 0,
	dividendsMonthlyMinor: 0,
	salaryMonthlyMinor: 0,
	jointExpensesMonthlyMinor: 0,
};

const MIN_TRANSFER_MINOR = 500;
const MIN_MATCHES_FOR_DB_ESTIMATE = 2;

// Account IDs are now derived from config at runtime
function getBusinessAccountIds(): AssetAccountId[] {
	return getAccountIdsByGroup('business') as AssetAccountId[];
}

function getPersonalAccountId(): AssetAccountId | undefined {
	return getFirstAccountIdByGroup('personal') as AssetAccountId | undefined;
}

function getJointAccountId(): AssetAccountId | undefined {
	return getFirstAccountIdByGroup('joint') as AssetAccountId | undefined;
}

const SCENARIO_CACHE_TTL_MS = 60_000;
let scenarioFlowsCache: {
	key: string;
	value: ScenarioMonthlyFlows;
	expires: number;
} | null = null;

function getCacheKey(config: ScenarioConfig): string {
	return JSON.stringify(config);
}

export type TransferRow = {
	id: string;
	chart_account_id: AssetAccountId;
	posted_at: string;
	amount_minor: number;
};

export type TransferPair = {
	from: TransferRow;
	to: TransferRow;
	absAmountMinor: number;
};

type TransferDbRow = {
	id: string;
	chart_account_id: string;
	posted_at: string;
	amount_minor: number;
};

function dateOnly(iso: string): string {
	return iso.slice(0, 10);
}

function previousDate(date: string): string {
	const d = new Date(date);
	d.setDate(d.getDate() - 1);
	return d.toISOString().slice(0, 10);
}

function pushByDay(map: Map<string, TransferRow[]>, day: string, txn: TransferRow) {
	const list = map.get(day) ?? [];
	list.push(txn);
	map.set(day, list);
}

function popByDay(map: Map<string, TransferRow[]>, day: string): TransferRow | null {
	const list = map.get(day);
	if (!list || list.length === 0) {
		return null;
	}

	const txn = list.pop() ?? null;
	if (list.length === 0) {
		map.delete(day);
	}
	return txn;
}

function groupTransfersByAbsAmount(rows: TransferRow[]): Map<number, TransferRow[]> {
	const byAmount = new Map<number, TransferRow[]>();

	for (const row of rows) {
		const absAmountMinor = Math.abs(row.amount_minor);
		if (absAmountMinor < MIN_TRANSFER_MINOR) {
			continue;
		}

		const group = byAmount.get(absAmountMinor) ?? [];
		group.push(row);
		byAmount.set(absAmountMinor, group);
	}

	return byAmount;
}

function getTransferDirection(txn: TransferRow, fromAccountIds: Set<AssetAccountId>, toAccountIds: Set<AssetAccountId>): 'from' | 'to' | null {
	if (txn.amount_minor < 0 && fromAccountIds.has(txn.chart_account_id)) {
		return 'from';
	}
	if (txn.amount_minor > 0 && toAccountIds.has(txn.chart_account_id)) {
		return 'to';
	}
	return null;
}

export function matchTransferPairsForAmountGroup(group: TransferRow[], fromAccountIds: Set<AssetAccountId>, toAccountIds: Set<AssetAccountId>): TransferPair[] {
	group.sort((a, b) => a.posted_at.localeCompare(b.posted_at));

	const pairs: TransferPair[] = [];
	const unmatchedFrom = new Map<string, TransferRow[]>();
	const unmatchedTo = new Map<string, TransferRow[]>();

	for (const txn of group) {
		const direction = getTransferDirection(txn, fromAccountIds, toAccountIds);
		if (!direction) {
			continue;
		}

		const day = dateOnly(txn.posted_at);
		const prev = previousDate(day);

		if (direction === 'from') {
			const match = popByDay(unmatchedTo, day) ?? popByDay(unmatchedTo, prev);
			if (!match) {
				pushByDay(unmatchedFrom, day, txn);
				continue;
			}

			pairs.push({
				from: txn,
				to: match,
				absAmountMinor: Math.abs(txn.amount_minor),
			});
			continue;
		}

		const match = popByDay(unmatchedFrom, day) ?? popByDay(unmatchedFrom, prev);
		if (!match) {
			pushByDay(unmatchedTo, day, txn);
			continue;
		}

		pairs.push({
			from: match,
			to: txn,
			absAmountMinor: Math.abs(txn.amount_minor),
		});
	}

	return pairs;
}

export function matchTransferPairs(rows: TransferRow[], fromAccountIds: Set<AssetAccountId>, toAccountIds: Set<AssetAccountId>): TransferPair[] {
	const pairs: TransferPair[] = [];

	for (const group of groupTransfersByAbsAmount(rows).values()) {
		pairs.push(...matchTransferPairsForAmountGroup(group, fromAccountIds, toAccountIds));
	}

	return pairs;
}

function listTrailingMonths(lookbackMonths: number, now = new Date()): string[] {
	const count = Math.max(1, Math.trunc(lookbackMonths));
	const start = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 1));
	start.setUTCMonth(start.getUTCMonth() - (count - 1));

	const months: string[] = [];
	for (let i = 0; i < count; i++) {
		const d = new Date(start);
		d.setUTCMonth(start.getUTCMonth() + i);
		months.push(d.toISOString().slice(0, 7));
	}
	return months;
}

export function median(values: number[]): number {
	if (values.length === 0) {
		return 0;
	}
	const sorted = [...values].sort((a, b) => a - b);
	const mid = Math.floor(sorted.length / 2);
	const a = sorted[mid - 1];
	const b = sorted[mid];
	if (sorted.length % 2 === 1) {
		return sorted[mid] ?? 0;
	}
	if (typeof a !== 'number' || typeof b !== 'number') {
		return 0;
	}
	return Math.round((a + b) / 2);
}

export function mean(values: number[]): number {
	if (values.length === 0) {
		return 0;
	}
	let sum = 0;
	for (const v of values) {
		sum += v;
	}
	return Math.round(sum / values.length);
}

export function getTypicalMonthlyMinor(totals: number[]): number {
	const med = median(totals);
	if (med > 0) {
		return med;
	}
	return mean(totals);
}

export function loadTransferRows(db: Database, chartAccountIds: AssetAccountId[], fromDate: string): TransferRow[] {
	if (chartAccountIds.length === 0) {
		return [];
	}

	const placeholders = chartAccountIds.map(() => '?').join(', ');

	// Query finds postings from journal entries where:
	// 1. Entry has exactly 2 total postings
	// 2. Both postings are to asset accounts
	// 3. At least one posting is to a requested account
	// 4. Entry is after fromDate
	const sql = `
		WITH two_posting_entries AS (
			SELECT journal_entry_id
			FROM postings
			GROUP BY journal_entry_id
			HAVING COUNT(*) = 2
		),
		transfer_entries AS (
			SELECT tpe.journal_entry_id AS journal_id
			FROM two_posting_entries tpe
			INNER JOIN postings p ON p.journal_entry_id = tpe.journal_entry_id
			INNER JOIN chart_of_accounts coa ON p.account_id = coa.id
			INNER JOIN journal_entries je ON je.id = tpe.journal_entry_id
			WHERE coa.account_type = 'asset'
			  AND je.posted_at >= ?
			GROUP BY tpe.journal_entry_id
			HAVING COUNT(*) = 2
		)
		SELECT
			p.id,
			p.account_id AS chart_account_id,
			je.posted_at,
			p.amount_minor
		FROM transfer_entries te
		INNER JOIN journal_entries je ON te.journal_id = je.id
		INNER JOIN postings p ON p.journal_entry_id = je.id
		WHERE p.account_id IN (${placeholders})
	`;

	const params = [fromDate, ...chartAccountIds];
	const rows = db.query<TransferDbRow, string[]>(sql).all(...params);

	return rows.map((row: TransferDbRow) => ({
		id: row.id,
		chart_account_id: row.chart_account_id as AssetAccountId,
		posted_at: row.posted_at,
		amount_minor: row.amount_minor,
	}));
}

type SalaryDividendResult = {
	salaryByMonth: Map<string, number>;
	dividendsByMonth: Map<string, number>;
	salaryMatches: number;
	dividendsMatches: number;
};

function categorizeSalaryDividendTransfers(pairs: TransferPair[], splitThresholdMinor: number): SalaryDividendResult {
	const salaryByMonth = new Map<string, number>();
	const dividendsByMonth = new Map<string, number>();
	let salaryMatches = 0;
	let dividendsMatches = 0;

	for (const pair of pairs) {
		const month = pair.to.posted_at.slice(0, 7);
		if (month.length !== 7) {
			continue;
		}

		if (pair.absAmountMinor >= splitThresholdMinor) {
			dividendsMatches += 1;
			dividendsByMonth.set(month, (dividendsByMonth.get(month) ?? 0) + pair.absAmountMinor);
		} else {
			salaryMatches += 1;
			salaryByMonth.set(month, (salaryByMonth.get(month) ?? 0) + pair.absAmountMinor);
		}
	}

	return { salaryByMonth, dividendsByMonth, salaryMatches, dividendsMatches };
}

type JointTransferResult = {
	jointByMonth: Map<string, number>;
	jointMatches: number;
};

function categorizeJointTransfers(pairs: TransferPair[]): JointTransferResult {
	const jointByMonth = new Map<string, number>();
	let jointMatches = 0;

	for (const pair of pairs) {
		const month = pair.from.posted_at.slice(0, 7);
		if (month.length !== 7) {
			continue;
		}

		jointMatches += 1;
		jointByMonth.set(month, (jointByMonth.get(month) ?? 0) + pair.absAmountMinor);
	}

	return { jointByMonth, jointMatches };
}

function computeTypicalFlow(matches: number, totals: number[], fallback: number): number {
	const value = matches >= MIN_MATCHES_FOR_DB_ESTIMATE ? getTypicalMonthlyMinor(totals) : fallback;
	return Math.max(0, value);
}

export function getScenarioMonthlyFlows(db: Database, config: Partial<ScenarioConfig> = {}): ScenarioMonthlyFlows {
	const mergedConfig: ScenarioConfig = { ...DEFAULT_SCENARIO_CONFIG, ...config };

	const cacheKey = getCacheKey(mergedConfig);
	const now = Date.now();

	if (scenarioFlowsCache && scenarioFlowsCache.key === cacheKey && scenarioFlowsCache.expires > now) {
		return scenarioFlowsCache.value;
	}

	const months = listTrailingMonths(mergedConfig.lookbackMonths);
	const fromDate = `${months[0] ?? new Date().toISOString().slice(0, 7)}-01`;

	// Get account IDs from config
	const businessAccountIds = getBusinessAccountIds();
	const personalAccountId = getPersonalAccountId();
	const jointAccountId = getJointAccountId();

	// Early return if no accounts configured
	if (businessAccountIds.length === 0 || !personalAccountId) {
		return {
			salaryMonthlyMinor: mergedConfig.salaryMonthlyMinor,
			dividendsMonthlyMinor: mergedConfig.dividendsMonthlyMinor,
			jointExpensesMonthlyMinor: mergedConfig.jointExpensesMonthlyMinor,
		};
	}

	const businessToPersonalRows = loadTransferRows(db, [...businessAccountIds, personalAccountId], fromDate);
	const businessToPersonalPairs = matchTransferPairs(businessToPersonalRows, new Set(businessAccountIds), new Set([personalAccountId]));
	const { salaryByMonth, dividendsByMonth, salaryMatches, dividendsMatches } = categorizeSalaryDividendTransfers(businessToPersonalPairs, mergedConfig.salaryDividendSplitMinor);

	// Joint transfer detection (only if joint account exists)
	let jointByMonth = new Map<string, number>();
	let jointMatches = 0;
	if (jointAccountId) {
		const jointTransferRows = loadTransferRows(db, [personalAccountId, jointAccountId], fromDate);
		const jointPairs = matchTransferPairs(jointTransferRows, new Set([personalAccountId]), new Set([jointAccountId]));
		const jointResult = categorizeJointTransfers(jointPairs);
		jointByMonth = jointResult.jointByMonth;
		jointMatches = jointResult.jointMatches;
	}

	const salaryTotals = months.map((m) => salaryByMonth.get(m) ?? 0);
	const dividendTotals = months.map((m) => dividendsByMonth.get(m) ?? 0);
	const jointTotals = months.map((m) => jointByMonth.get(m) ?? 0);

	const result: ScenarioMonthlyFlows = {
		salaryMonthlyMinor: computeTypicalFlow(salaryMatches, salaryTotals, mergedConfig.salaryMonthlyMinor),
		dividendsMonthlyMinor: computeTypicalFlow(dividendsMatches, dividendTotals, mergedConfig.dividendsMonthlyMinor),
		jointExpensesMonthlyMinor: computeTypicalFlow(jointMatches, jointTotals, mergedConfig.jointExpensesMonthlyMinor),
	};

	scenarioFlowsCache = { key: cacheKey, value: result, expires: now + SCENARIO_CACHE_TTL_MS };
	return result;
}

export function getScenarioCashflowDelta(groupId: GroupId, toggles: ScenarioToggles, flows: ScenarioMonthlyFlows): ScenarioCashflowDelta {
	if (groupId === 'business') {
		return {
			incomeMinorDelta: 0,
			expenseMinorDelta: (toggles.includeDividends ? flows.dividendsMonthlyMinor : 0) + (toggles.includeSalary ? flows.salaryMonthlyMinor : 0),
		};
	}

	if (groupId === 'personal') {
		return {
			incomeMinorDelta: (toggles.includeDividends ? flows.dividendsMonthlyMinor : 0) + (toggles.includeSalary ? flows.salaryMonthlyMinor : 0),
			expenseMinorDelta: toggles.includeJointExpenses ? flows.jointExpensesMonthlyMinor : 0,
		};
	}

	return { incomeMinorDelta: 0, expenseMinorDelta: 0 };
}

export function getScenarioTaxNetDelta(groupId: GroupId, toggles: ScenarioToggles, flows: ScenarioMonthlyFlows): number {
	if (groupId === 'business') {
		return toggles.includeSalary ? -flows.salaryMonthlyMinor : 0;
	}

	if (groupId === 'personal') {
		return (
			(toggles.includeDividends ? flows.dividendsMonthlyMinor : 0) + (toggles.includeSalary ? flows.salaryMonthlyMinor : 0) - (toggles.includeJointExpenses ? flows.jointExpensesMonthlyMinor : 0)
		);
	}

	return 0;
}

export function applyScenarioToCashflowSeries(series: MonthlyCashflowPoint[], groupId: GroupId, toggles: ScenarioToggles, flows: ScenarioMonthlyFlows): MonthlyCashflowPoint[] {
	const delta = getScenarioCashflowDelta(groupId, toggles, flows);
	if (delta.incomeMinorDelta === 0 && delta.expenseMinorDelta === 0) {
		return series;
	}

	return series.map((point) => {
		const incomeMinor = point.incomeMinor + delta.incomeMinorDelta;
		const expenseMinor = point.expenseMinor + delta.expenseMinorDelta;
		const netMinor = incomeMinor - expenseMinor;
		const savingsRatePct = incomeMinor > 0 ? Math.round((netMinor / incomeMinor) * 1000) / 10 : null;
		// Recalculate deviation ratio with new expense value
		const expenseDeviationRatio = point.rollingMedianExpenseMinor && point.rollingMedianExpenseMinor > 0 ? Math.round((expenseMinor / point.rollingMedianExpenseMinor) * 100) / 100 : null;
		return {
			...point,
			incomeMinor,
			expenseMinor,
			netMinor,
			savingsRatePct,
			expenseDeviationRatio,
		};
	});
}

// ============================================
// HEALTH (from health.ts)
// ============================================

export type HealthAssumptions = {
	trailingExpenseWindowMonths: number;
	expenseReserveMonths: number;
	corpTaxRate: number;
	personalIncomeTaxRate: number;
	taxYearStartMonth: number;
};

export type GroupHealthPoint = {
	date: string;
	healthMinor: number;
};

export function getGroupDailyHealthSeries(
	db: Database,
	groupId: GroupId,
	options: BalanceSeriesOptions = {},
	assumptions: Partial<HealthAssumptions> = {},
	scenario?: ScenarioToggles,
	scenarioConfig: Partial<ScenarioConfig> = {},
): GroupHealthPoint[] {
	const ledgerScenarioConfig: Partial<LedgerScenarioConfig> = {};
	if (scenarioConfig.dividendsMonthlyMinor !== undefined) ledgerScenarioConfig.dividendsMonthlyMinor = scenarioConfig.dividendsMonthlyMinor;
	if (scenarioConfig.salaryMonthlyMinor !== undefined) ledgerScenarioConfig.salaryMonthlyMinor = scenarioConfig.salaryMonthlyMinor;
	if (scenarioConfig.jointExpensesMonthlyMinor !== undefined) ledgerScenarioConfig.jointExpensesMonthlyMinor = scenarioConfig.jointExpensesMonthlyMinor;
	return getLedgerGroupDailyHealthSeries(db, groupId as LedgerGroupId, options, assumptions, scenario, ledgerScenarioConfig);
}

// ============================================
// RUNWAY (from runway.ts)
// ============================================

export type RunwayAssumptions = {
	trailingOutflowWindowMonths: number;
	maxRunwayMonths: number;
};

export type GroupRunwayPoint = {
	date: string;
	runwayMonths: number;
	isNetPositive?: boolean;
	medianExpenseMinor?: number;
};

export function getGroupDailyRunwaySeries(
	db: Database,
	groupId: GroupId,
	options: BalanceSeriesOptions = {},
	assumptions: Partial<RunwayAssumptions> = {},
	scenario?: ScenarioToggles,
	scenarioConfig: Partial<ScenarioConfig> = {},
): GroupRunwayPoint[] {
	const ledgerScenarioConfig: Partial<LedgerScenarioConfig> = {};
	if (scenarioConfig.dividendsMonthlyMinor !== undefined) ledgerScenarioConfig.dividendsMonthlyMinor = scenarioConfig.dividendsMonthlyMinor;
	if (scenarioConfig.salaryMonthlyMinor !== undefined) ledgerScenarioConfig.salaryMonthlyMinor = scenarioConfig.salaryMonthlyMinor;
	if (scenarioConfig.jointExpensesMonthlyMinor !== undefined) ledgerScenarioConfig.jointExpensesMonthlyMinor = scenarioConfig.jointExpensesMonthlyMinor;
	return getLedgerGroupDailyRunwaySeries(db, groupId as LedgerGroupId, options, assumptions, scenario, ledgerScenarioConfig);
}

// ============================================
// RESERVE BREAKDOWN (for stacked area visualization)
// ============================================

export type GroupReserveBreakdownPoint = {
	date: string;
	balanceMinor: number;
	taxReserveMinor: number;
	expenseReserveMinor: number;
	availableMinor: number;
};

export function getGroupDailyReserveBreakdownSeries(
	db: Database,
	groupId: GroupId,
	options: BalanceSeriesOptions = {},
	assumptions: Partial<HealthAssumptions> = {},
	scenario?: ScenarioToggles,
	scenarioConfig: Partial<ScenarioConfig> = {},
): GroupReserveBreakdownPoint[] {
	const ledgerScenarioConfig: Partial<LedgerScenarioConfig> = {};
	if (scenarioConfig.dividendsMonthlyMinor !== undefined) ledgerScenarioConfig.dividendsMonthlyMinor = scenarioConfig.dividendsMonthlyMinor;
	if (scenarioConfig.salaryMonthlyMinor !== undefined) ledgerScenarioConfig.salaryMonthlyMinor = scenarioConfig.salaryMonthlyMinor;
	if (scenarioConfig.jointExpensesMonthlyMinor !== undefined) ledgerScenarioConfig.jointExpensesMonthlyMinor = scenarioConfig.jointExpensesMonthlyMinor;
	return getLedgerGroupDailyReserveBreakdownSeries(db, groupId as LedgerGroupId, options, assumptions, scenario, ledgerScenarioConfig);
}

// ============================================
// CONSOLIDATED RUNWAY (cross-group)
// ============================================

export type { ConsolidatedRunwayOptions, ConsolidatedRunwayPoint };

export function getConsolidatedDailyRunwaySeries(
	db: Database,
	options: { includeGroups: string[]; from?: string; to?: string; limit?: number },
	assumptions: Partial<RunwayAssumptions> = {},
): ConsolidatedRunwayPoint[] {
	const consolidatedOptions: ConsolidatedRunwayOptions = { includeGroups: options.includeGroups };
	if (options.from !== undefined) consolidatedOptions.from = options.from;
	if (options.to !== undefined) consolidatedOptions.to = options.to;
	if (options.limit !== undefined) consolidatedOptions.limit = options.limit;
	return getLedgerConsolidatedDailyRunwaySeries(db, consolidatedOptions, assumptions);
}
