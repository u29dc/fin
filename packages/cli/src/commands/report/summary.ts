/**
 * `report summary` -- Comprehensive Markdown financial summary across all groups.
 *
 * Text mode: generates the same markdown output as the original summary.ts.
 * JSON mode: returns structured financial data (implemented in ENG-013).
 *
 * Outputs balances, runway, health, cashflow, and expense categories
 * for each group (ordered per config), plus a consolidated section.
 */

import { writeFileSync } from 'node:fs';

import {
	type ExpenseNode,
	type GroupId,
	getBalanceSheet,
	getConsolidatedDailyRunwaySeries,
	getGroupCashFlowDataMedian,
	getGroupDailyReserveBreakdownSeries,
	getGroupDailyRunwaySeries,
	getGroupExpenseHierarchyMedian,
	getGroupExpenseTreeMedian,
	getGroupMonthlyCashflowSeries,
	getLatestBalances,
	isGroupId,
	type SankeyFlowData,
} from '@fin/core';
import { getAccountIdsByGroup, getAccountsByGroup, getBurnRateExcludeAccounts, getGroupIds, getGroupMetadata, getLiquidAccountIds } from '@fin/core/config';

import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount, formatMonths, formatPercentRaw } from '../../format';
import { log } from '../../logger';
import { defineToolCommand } from '../../tool';

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

function today(): string {
	return new Date().toISOString().slice(0, 10);
}

function monthsAgo(n: number): string {
	const d = new Date();
	d.setMonth(d.getMonth() - n);
	return d.toISOString().slice(0, 10);
}

// ---------------------------------------------------------------------------
// Markdown helpers
// ---------------------------------------------------------------------------

function mdTable(headers: string[], alignments: ('left' | 'right')[], rows: string[][]): string[] {
	const lines: string[] = [];
	lines.push(`| ${headers.join(' | ')} |`);
	lines.push(`|${alignments.map((a) => (a === 'right' ? '---:' : '---')).join('|')}|`);
	for (const row of rows) {
		lines.push(`| ${row.join(' | ')} |`);
	}
	return lines;
}

function last<T>(arr: T[]): T | undefined {
	return arr.length > 0 ? arr[arr.length - 1] : undefined;
}

// ---------------------------------------------------------------------------
// Expense tree helpers
// ---------------------------------------------------------------------------

function flattenExpenseTree(nodes: ExpenseNode[], depth = 0, excludePrefixes: string[] = []): Array<{ name: string; amount: number; depth: number; excluded: boolean }> {
	const result: Array<{ name: string; amount: number; depth: number; excluded: boolean }> = [];
	const sorted = [...nodes].sort((a, b) => b.totalMinor - a.totalMinor);
	for (const node of sorted) {
		const excluded = excludePrefixes.some((prefix) => node.accountId === prefix || node.accountId.startsWith(`${prefix}:`));
		result.push({ name: node.name, amount: node.totalMinor, depth, excluded });
		if (node.children.length > 0) {
			result.push(...flattenExpenseTree(node.children, depth + 1, excludePrefixes));
		}
	}
	return result;
}

function renderExpenseBreakdown(expenseTree: ExpenseNode[], heading: string): string[] {
	if (expenseTree.length === 0) return [];
	const excludePrefixes = getBurnRateExcludeAccounts();
	const flat = flattenExpenseTree(expenseTree, 0, excludePrefixes);
	const treeRows: string[][] = flat.map((e) => {
		const indent = e.depth > 0 ? `${'--'.repeat(e.depth)} ` : '';
		const suffix = e.excluded ? ' *' : '';
		return [`${indent}${e.name}${suffix}`, formatAmount(e.amount)];
	});

	const lines: string[] = [];
	lines.push(heading);
	lines.push(...mdTable(['Category', 'Monthly Avg'], ['left', 'right'], treeRows));
	if (flat.some((e) => e.excluded)) {
		lines.push('');
		lines.push('\\* excluded from burn rate');
	}
	lines.push('');
	return lines;
}

// ---------------------------------------------------------------------------
// Section renderers
// ---------------------------------------------------------------------------

function renderFlowSection(db: ReturnType<typeof getReadonlyDb>, groupId: GroupId, months: number): string[] {
	const flowData: SankeyFlowData = getGroupCashFlowDataMedian(db, groupId, { months });
	if (flowData.links.length === 0) return [];

	const lines: string[] = [];

	const nodeCategories = new Map<string, string>();
	for (const n of flowData.nodes) {
		nodeCategories.set(n.name, n.category);
	}

	const incomeSources = flowData.links.filter((l) => nodeCategories.get(l.source) === 'income').sort((a, b) => b.value - a.value);

	const expenseSinks = flowData.links.filter((l) => nodeCategories.get(l.target) === 'expense').sort((a, b) => b.value - a.value);

	if (incomeSources.length > 0) {
		lines.push('### Income Sources (monthly avg)');
		const rows = incomeSources.map((l) => [l.source, formatAmount(l.value)]);
		lines.push(...mdTable(['Source', 'Monthly Avg'], ['left', 'right'], rows));
		lines.push('');
	}

	if (expenseSinks.length > 0) {
		lines.push('### Expense Sinks (monthly avg)');
		const rows = expenseSinks.map((l) => [l.target, formatAmount(l.value)]);
		lines.push(...mdTable(['Category', 'Monthly Avg'], ['left', 'right'], rows));
		lines.push('');
	}

	return lines;
}

function renderBalanceSheetSection(db: ReturnType<typeof getReadonlyDb>): string[] {
	const bs = getBalanceSheet(db);
	const lines: string[] = [];

	lines.push('## Balance Sheet');
	const rows: string[][] = [
		['Assets', formatAmount(bs.assets)],
		['Liabilities', formatAmount(bs.liabilities)],
		['Net Worth', formatAmount(bs.netWorth)],
		['Income (YTD)', formatAmount(bs.income)],
		['Expenses (YTD)', formatAmount(bs.expenses)],
		['Net Income (YTD)', formatAmount(bs.netIncome)],
		['Equity', formatAmount(bs.equity)],
	];
	lines.push(...mdTable(['Metric', 'Value'], ['left', 'right'], rows));
	lines.push('');

	return lines;
}

// ---------------------------------------------------------------------------
// Asset allocation helpers
// ---------------------------------------------------------------------------

type BalanceEntry = { chartAccountId: string; balanceMinor: number | null };
type ReservePoint = { taxReserveMinor: number; expenseReserveMinor: number; availableMinor: number; balanceMinor: number };

function pctOf(value: number, total: number): string {
	return total > 0 ? `${Math.round((value / total) * 100)}` : '-';
}

function personalAllocationRows(groupId: GroupId, balances: BalanceEntry[], medianExpense: number | null): string[][] {
	const accounts = getAccountsByGroup(groupId);
	const balanceMap = new Map<string, number>();
	for (const b of balances) {
		if (b.balanceMinor !== null) balanceMap.set(b.chartAccountId, b.balanceMinor);
	}

	let checkingTotal = 0;
	let savingsTotal = 0;
	let investmentTotal = 0;

	for (const acc of accounts) {
		const bal = balanceMap.get(acc.id) ?? 0;
		if (acc.subtype === 'investment') investmentTotal += bal;
		else if (acc.subtype === 'savings') savingsTotal += bal;
		else checkingTotal += bal;
	}

	const expenseBuffer = medianExpense !== null ? medianExpense * 3 : 0;
	const available = Math.max(0, checkingTotal - expenseBuffer);
	const total = checkingTotal + savingsTotal + investmentTotal;

	return [
		['Available cash', formatAmount(available), pctOf(available, total)],
		['Expense buffer', formatAmount(expenseBuffer), pctOf(expenseBuffer, total)],
		['Emergency fund', formatAmount(savingsTotal), pctOf(savingsTotal, total)],
		['Investment', formatAmount(investmentTotal), pctOf(investmentTotal, total)],
	];
}

function reserveAllocationRows(reserve: ReservePoint): string[][] {
	const total = reserve.balanceMinor;
	return [
		['Available', formatAmount(reserve.availableMinor), pctOf(reserve.availableMinor, total)],
		['Expense buffer', formatAmount(reserve.expenseReserveMinor), pctOf(reserve.expenseReserveMinor, total)],
		['Tax reserve', formatAmount(reserve.taxReserveMinor), pctOf(reserve.taxReserveMinor, total)],
	];
}

function renderAssetAllocation(groupId: GroupId, balances: BalanceEntry[], medianExpense: number | null, latestReserve: ReservePoint | undefined): string[] {
	const meta = getGroupMetadata(groupId);
	const rows = meta.taxType === 'income' ? personalAllocationRows(groupId, balances, medianExpense) : latestReserve ? reserveAllocationRows(latestReserve) : [];

	if (rows.length === 0) return [];

	const lines: string[] = [];
	lines.push('### Asset Allocation');
	lines.push(...mdTable(['Segment', 'Amount', '%'], ['left', 'right', 'right'], rows));
	return lines;
}

// ---------------------------------------------------------------------------
// Last month / cashflow section
// ---------------------------------------------------------------------------

type CashflowPoint = { month: string; incomeMinor: number; expenseMinor: number; netMinor: number; savingsRatePct: number | null };

function momChange(current: number, previous: number): number | null {
	return previous !== 0 ? Math.round(((current - previous) / Math.abs(previous)) * 100) : null;
}

function fmtMom(v: number | null): string {
	return v !== null ? `${v >= 0 ? '+' : ''}${v}%` : '-';
}

function renderLastMonth(current: CashflowPoint, previous: CashflowPoint | undefined): string[] {
	const monthLabel = current.month.slice(0, 7);
	const momIncome = previous ? momChange(current.incomeMinor, previous.incomeMinor) : null;
	const momExpense = previous ? momChange(current.expenseMinor, previous.expenseMinor) : null;
	const momNet = previous ? momChange(current.netMinor, previous.netMinor) : null;
	const savingsNote = current.savingsRatePct !== null ? ` (${formatPercentRaw(current.savingsRatePct, 0)} rate)` : '';

	const rows: string[][] = [];
	rows.push(['Income', formatAmount(current.incomeMinor), fmtMom(momIncome)]);
	rows.push(['Expenses', formatAmount(current.expenseMinor), fmtMom(momExpense)]);
	rows.push(['Net', formatAmount(current.netMinor), `${fmtMom(momNet)}${savingsNote}`]);

	const lines: string[] = [];
	lines.push(`### Last Month (${monthLabel})`);
	lines.push(...mdTable(['Metric', 'Value', 'MoM'], ['left', 'right', 'right'], rows));
	lines.push('');
	return lines;
}

// ---------------------------------------------------------------------------
// Group section renderer
// ---------------------------------------------------------------------------

function renderGroupSection(db: ReturnType<typeof getReadonlyDb>, groupId: GroupId, months: number): string[] {
	const meta = getGroupMetadata(groupId);
	const accountIds = getAccountIdsByGroup(groupId);
	if (accountIds.length === 0) return [];

	const lines: string[] = [];
	lines.push(`## ${meta.label}`);
	lines.push('');

	const balances = getLatestBalances(db, accountIds);
	let totalMinor = 0;
	const balanceRows: string[][] = [];
	for (const b of balances) {
		const bal = b.balanceMinor;
		if (bal !== null) totalMinor += bal;
		balanceRows.push([b.chartAccountId, bal !== null ? formatAmount(bal) : '-']);
	}
	balanceRows.push(['**Total**', `**${formatAmount(totalMinor)}**`]);

	lines.push('### Balances');
	lines.push(...mdTable(['Account', 'Balance'], ['left', 'right'], balanceRows));
	lines.push('');

	lines.push(...renderSnapshotSection(db, groupId, months, balances, totalMinor));

	return lines;
}

function renderSnapshotSection(db: ReturnType<typeof getReadonlyDb>, groupId: GroupId, months: number, balances: BalanceEntry[], totalMinor: number): string[] {
	const lines: string[] = [];

	const runwaySeries = getGroupDailyRunwaySeries(db, groupId);
	const reserveSeries = getGroupDailyReserveBreakdownSeries(db, groupId);

	const latestRunway = last(runwaySeries);
	const latestReserve = last(reserveSeries);
	const medianExpense = latestRunway?.medianExpenseMinor ?? null;

	const from = monthsAgo(months);
	const cashflow = getGroupMonthlyCashflowSeries(db, groupId, { from });

	const lastCompleteMonth = cashflow.length >= 2 ? cashflow[cashflow.length - 2] : undefined;

	const snapshotRows: string[][] = [];
	snapshotRows.push(['Runway', latestRunway ? formatMonths(latestRunway.runwayMonths) : 'N/A']);
	snapshotRows.push(['Last month', lastCompleteMonth ? formatAmount(lastCompleteMonth.netMinor) : 'N/A']);
	snapshotRows.push(['Net worth', formatAmount(totalMinor)]);
	snapshotRows.push(['Med spend', medianExpense !== null ? formatAmount(medianExpense) : 'N/A']);

	lines.push('### Snapshot');
	lines.push(...mdTable(['Metric', 'Value'], ['left', 'right'], snapshotRows));
	lines.push('');

	lines.push(...renderAssetAllocation(groupId, balances, medianExpense, latestReserve));
	lines.push('');

	if (lastCompleteMonth) {
		const prevMonth = cashflow.length >= 3 ? cashflow[cashflow.length - 3] : undefined;
		lines.push(...renderLastMonth(lastCompleteMonth, prevMonth));
	}

	if (cashflow.length > 0) {
		const cfRows: string[][] = cashflow.map((p) => [p.month.slice(0, 7), formatAmount(p.incomeMinor), formatAmount(p.expenseMinor), formatAmount(p.netMinor), formatPercentRaw(p.savingsRatePct)]);

		lines.push(`### Cashflow (${months} months)`);
		lines.push(...mdTable(['Month', 'Income', 'Expenses', 'Net', 'Savings%'], ['left', 'right', 'right', 'right', 'right'], cfRows));
		lines.push('');
	}

	lines.push(...renderFlowSection(db, groupId, months));

	const expenseTree = getGroupExpenseTreeMedian(db, groupId, { months });
	lines.push(...renderExpenseBreakdown(expenseTree, '### Expense Breakdown (monthly avg)'));

	return lines;
}

// ---------------------------------------------------------------------------
// Consolidated section
// ---------------------------------------------------------------------------

function renderConsolidatedSection(db: ReturnType<typeof getReadonlyDb>, groupIds: GroupId[], groupTotals: Map<GroupId, number>, months: number): string[] {
	const lines: string[] = [];

	let totalBalance = 0;
	for (const t of groupTotals.values()) {
		totalBalance += t;
	}

	const consolidated = getConsolidatedDailyRunwaySeries(db, { includeGroups: groupIds });
	const latestCon = last(consolidated);

	const rows: string[][] = [];
	rows.push(['Total balance', formatAmount(totalBalance)]);
	rows.push(['Consolidated runway', latestCon ? formatMonths(latestCon.runwayMonths) : 'N/A']);
	rows.push(['Burn rate/mo', latestCon ? formatAmount(latestCon.burnRateMinor) : 'N/A']);

	lines.push('## Consolidated');
	lines.push(...mdTable(['Metric', 'Value'], ['left', 'right'], rows));
	lines.push('');

	const allLiquidAccountIds = getLiquidAccountIds();
	const expenseTree = getGroupExpenseHierarchyMedian(db, allLiquidAccountIds, { months });
	lines.push(...renderExpenseBreakdown(expenseTree, '### Expense Breakdown (monthly avg)'));

	return lines;
}

// ---------------------------------------------------------------------------
// Methodology section
// ---------------------------------------------------------------------------

function renderMethodology(groupIds: GroupId[]): string[] {
	const accountLines: string[] = [];
	for (const gid of groupIds) {
		const meta = getGroupMetadata(gid);
		const ids = getAccountIdsByGroup(gid);
		if (ids.length > 0) {
			accountLines.push(`  - ${meta.label}: ${ids.join(', ')}`);
		}
	}

	const excludePrefixes = getBurnRateExcludeAccounts();
	const excludeLines: string[] = [];
	if (excludePrefixes.length > 0) {
		excludeLines.push(
			`- **Burn rate exclusions**: the following account prefixes are excluded from consolidated burn rate and runway via \`burn_rate_exclude_accounts\`: ${excludePrefixes.map((p) => `\`${p}\``).join(', ')}. These represent pass-through expenses (e.g., VAT) that inflate burn without reflecting real spending.`,
		);
	}

	return [
		'---',
		'',
		'## Methodology',
		'- **Runway**: balance / trailing 6-month median outflow (configurable via `burn_rate_method`). Non-liquid accounts (investments) excluded. Capped at 120 mo if net positive.',
		"- **Median expense**: 6-month rolling median of prior months' expenses. Requires 3+ months of data.",
		'- **Asset allocation (personal)**: splits balances by account subtype (checking/savings/investment). Expense buffer = median expense * 3. Available = max(0, checking - buffer).',
		'- **Asset allocation (other)**: uses reserve breakdown series. Available, expense buffer, and tax reserve from latest data point.',
		'- **Last month**: last complete calendar month. MoM change = ((current - previous) / |previous|) * 100.',
		'- **Expense reserve**: avgExpense * expense_reserve_months (group-configurable, default 3).',
		'- **Tax reserve**: max(0, ytdNet) * taxRate. Corp=25%, income=20%, none=0%. Resets at tax year start.',
		'- **Balance sheet**: point-in-time snapshot of assets, liabilities, equity, and YTD income/expenses across all accounts.',
		'- **Income/expense flows**: monthly average of flows between account categories over the period. Derived from Sankey flow data.',
		'- **Expense tree**: hierarchical breakdown of expenses using monthly averages. Children sorted by amount descending within each parent. Accounts excluded from burn rate are annotated with `*`.',
		'- **Consolidated expense breakdown**: hierarchical expense breakdown across all liquid accounts. Shows where consolidated burn goes.',
		'- **Cashflow**: most recent N months from today. Inferred from asset account postings; excludes pure asset-to-asset transfers.',
		'- **Savings rate**: net / income * 100. Null if no income in the month.',
		'- **Burn rate**: trailing 6-month median (default) or mean of external outflows, excluding configured pass-through accounts and inter-account transfers. Median resists spiky one-off expenses (travel, equipment). Configurable via `burn_rate_method` in config.',
		'- **Income categorization**: derived from user-defined rules in fin.rules.ts. Uncategorized sources appear as "Other".',
		...excludeLines,
		'- **Accounts tracked**:',
		...accountLines,
	];
}

// ---------------------------------------------------------------------------
// Group resolution + totals
// ---------------------------------------------------------------------------

function resolveGroupIds(group: string | undefined, jsonMode: boolean, start: number): GroupId[] {
	if (group) {
		if (!isGroupId(group)) {
			if (jsonMode) {
				fail('report.summary', 'INVALID_GROUP', `Invalid group: ${group}`, 'Use --group=personal, --group=business, or --group=joint', start);
			}
			process.stderr.write(`Error: Invalid group: ${group}. Use: personal, business, joint\n`);
			process.exit(1);
		}
		return [group];
	}
	return getGroupIds() as GroupId[];
}

function computeGroupTotals(db: ReturnType<typeof getReadonlyDb>, groupIds: GroupId[]): Map<GroupId, number> {
	const totals = new Map<GroupId, number>();
	for (const gid of groupIds) {
		const accountIds = getAccountIdsByGroup(gid);
		if (accountIds.length === 0) continue;
		const balances = getLatestBalances(db, accountIds);
		let total = 0;
		for (const b of balances) {
			if (b.balanceMinor !== null) total += b.balanceMinor;
		}
		totals.set(gid, total);
	}
	return totals;
}

// ---------------------------------------------------------------------------
// JSON data builders
// ---------------------------------------------------------------------------

function pctNum(value: number, total: number): number | null {
	return total > 0 ? Math.round((value / total) * 100) : null;
}

function buildPersonalAllocation(groupId: GroupId, balances: BalanceEntry[], medianExpense: number | null): Array<{ segment: string; amount: number; pct: number | null }> {
	const accounts = getAccountsByGroup(groupId);
	const balanceMap = new Map<string, number>();
	for (const b of balances) {
		if (b.balanceMinor !== null) balanceMap.set(b.chartAccountId, b.balanceMinor);
	}

	let checkingTotal = 0;
	let savingsTotal = 0;
	let investmentTotal = 0;

	for (const acc of accounts) {
		const bal = balanceMap.get(acc.id) ?? 0;
		if (acc.subtype === 'investment') investmentTotal += bal;
		else if (acc.subtype === 'savings') savingsTotal += bal;
		else checkingTotal += bal;
	}

	const expenseBuffer = medianExpense !== null ? medianExpense * 3 : 0;
	const available = Math.max(0, checkingTotal - expenseBuffer);
	const total = checkingTotal + savingsTotal + investmentTotal;

	return [
		{ segment: 'Available cash', amount: available, pct: pctNum(available, total) },
		{ segment: 'Expense buffer', amount: expenseBuffer, pct: pctNum(expenseBuffer, total) },
		{ segment: 'Emergency fund', amount: savingsTotal, pct: pctNum(savingsTotal, total) },
		{ segment: 'Investment', amount: investmentTotal, pct: pctNum(investmentTotal, total) },
	];
}

function buildReserveAllocation(reserve: ReservePoint): Array<{ segment: string; amount: number; pct: number | null }> {
	const total = reserve.balanceMinor;
	return [
		{ segment: 'Available', amount: reserve.availableMinor, pct: pctNum(reserve.availableMinor, total) },
		{ segment: 'Expense buffer', amount: reserve.expenseReserveMinor, pct: pctNum(reserve.expenseReserveMinor, total) },
		{ segment: 'Tax reserve', amount: reserve.taxReserveMinor, pct: pctNum(reserve.taxReserveMinor, total) },
	];
}

function buildAssetAllocation(
	groupId: GroupId,
	balances: BalanceEntry[],
	medianExpense: number | null,
	latestReserve: ReservePoint | undefined,
): Array<{ segment: string; amount: number; pct: number | null }> {
	const meta = getGroupMetadata(groupId);
	if (meta.taxType === 'income') return buildPersonalAllocation(groupId, balances, medianExpense);
	if (latestReserve) return buildReserveAllocation(latestReserve);
	return [];
}

function buildFlowData(
	db: ReturnType<typeof getReadonlyDb>,
	groupId: GroupId,
	months: number,
): { incomeSources: Array<{ source: string; amount: number }>; expenseSinks: Array<{ category: string; amount: number }> } {
	const flowData: SankeyFlowData = getGroupCashFlowDataMedian(db, groupId, { months });
	const nodeCategories = new Map<string, string>();
	for (const n of flowData.nodes) {
		nodeCategories.set(n.name, n.category);
	}

	const incomeSources = flowData.links
		.filter((l) => nodeCategories.get(l.source) === 'income')
		.sort((a, b) => b.value - a.value)
		.map((l) => ({ source: l.source, amount: l.value }));

	const expenseSinks = flowData.links
		.filter((l) => nodeCategories.get(l.target) === 'expense')
		.sort((a, b) => b.value - a.value)
		.map((l) => ({ category: l.target, amount: l.value }));

	return { incomeSources, expenseSinks };
}

function flattenExpenseTreeJson(
	nodes: ExpenseNode[],
): Array<{ accountId: string; name: string; amount: number; children: Array<{ accountId: string; name: string; amount: number; children: unknown[] }> }> {
	const sorted = [...nodes].sort((a, b) => b.totalMinor - a.totalMinor);
	return sorted.map((node) => ({
		accountId: node.accountId,
		name: node.name,
		amount: node.totalMinor,
		children: node.children.length > 0 ? flattenExpenseTreeJson(node.children) : [],
	}));
}

function buildLastMonth(cashflow: CashflowPoint[]): { income: number; expenses: number; net: number; momIncome: number | null; momExpenses: number | null } | null {
	if (cashflow.length < 2) return null;
	const current = cashflow[cashflow.length - 2] as CashflowPoint;
	const previous = cashflow.length >= 3 ? (cashflow[cashflow.length - 3] as CashflowPoint) : undefined;
	return {
		income: current.incomeMinor,
		expenses: current.expenseMinor,
		net: current.netMinor,
		momIncome: previous ? momChange(current.incomeMinor, previous.incomeMinor) : null,
		momExpenses: previous ? momChange(current.expenseMinor, previous.expenseMinor) : null,
	};
}

function buildGroupJson(db: ReturnType<typeof getReadonlyDb>, groupId: GroupId, months: number): Record<string, unknown> | null {
	const meta = getGroupMetadata(groupId);
	const accountIds = getAccountIdsByGroup(groupId);
	if (accountIds.length === 0) return null;

	const balances = getLatestBalances(db, accountIds);
	const runwaySeries = getGroupDailyRunwaySeries(db, groupId);
	const reserveSeries = getGroupDailyReserveBreakdownSeries(db, groupId);
	const latestRunway = last(runwaySeries);
	const latestReserve = last(reserveSeries);
	const medianExpense = latestRunway?.medianExpenseMinor ?? null;

	const from = monthsAgo(months);
	const cashflow = getGroupMonthlyCashflowSeries(db, groupId, { from });
	const lastCompleteMonth = cashflow.length >= 2 ? cashflow[cashflow.length - 2] : undefined;
	const { incomeSources, expenseSinks } = buildFlowData(db, groupId, months);
	const expenseTree = getGroupExpenseTreeMedian(db, groupId, { months });

	let totalMinor = 0;
	for (const b of balances) {
		if (b.balanceMinor !== null) totalMinor += b.balanceMinor;
	}

	return {
		id: groupId,
		label: meta.label,
		balances: balances.map((b) => ({ account: b.chartAccountId, balance: b.balanceMinor })),
		snapshot: {
			runway: latestRunway?.runwayMonths ?? null,
			lastMonthNet: lastCompleteMonth?.netMinor ?? null,
			netWorth: totalMinor,
			medianSpend: medianExpense,
		},
		assetAllocation: buildAssetAllocation(groupId, balances, medianExpense, latestReserve),
		lastMonth: buildLastMonth(cashflow),
		cashflow: cashflow.map((p) => ({ month: p.month, income: p.incomeMinor, expenses: p.expenseMinor, net: p.netMinor, savingsRate: p.savingsRatePct })),
		incomeSources,
		expenseSinks,
		expenseTree: flattenExpenseTreeJson(expenseTree),
	};
}

function buildConsolidatedJson(db: ReturnType<typeof getReadonlyDb>, groupIds: GroupId[], months: number): Record<string, unknown> {
	let totalBalance = 0;
	for (const gid of groupIds) {
		const accountIds = getAccountIdsByGroup(gid);
		const balances = getLatestBalances(db, accountIds);
		for (const b of balances) {
			if (b.balanceMinor !== null) totalBalance += b.balanceMinor;
		}
	}

	const consolidated = getConsolidatedDailyRunwaySeries(db, { includeGroups: groupIds });
	const latestCon = last(consolidated);

	const allLiquidAccountIds = getLiquidAccountIds();
	const expenseTree = getGroupExpenseHierarchyMedian(db, allLiquidAccountIds, { months });

	return {
		totalBalance,
		runway: latestCon?.runwayMonths ?? null,
		burnRate: latestCon?.burnRateMinor ?? null,
		expenseTree: flattenExpenseTreeJson(expenseTree),
	};
}

function buildSummaryJson(db: ReturnType<typeof getReadonlyDb>, groupIds: GroupId[], months: number): Record<string, unknown> {
	const groups: Array<Record<string, unknown>> = [];
	for (const gid of groupIds) {
		const groupData = buildGroupJson(db, gid, months);
		if (groupData) groups.push(groupData);
	}

	const consolidated = groupIds.length > 1 && groups.length > 1 ? buildConsolidatedJson(db, groupIds, months) : null;

	const bs = getBalanceSheet(db);

	return {
		generatedAt: today(),
		periodMonths: months,
		currency: 'GBP',
		groups,
		consolidated,
		balanceSheet: {
			assets: bs.assets,
			liabilities: bs.liabilities,
			equity: bs.equity,
			income: bs.income,
			expenses: bs.expenses,
			netWorth: bs.netWorth,
			netIncome: bs.netIncome,
		},
	};
}

// ---------------------------------------------------------------------------
// Markdown builder
// ---------------------------------------------------------------------------

function buildSummaryMarkdown(db: ReturnType<typeof getReadonlyDb>, groupIds: GroupId[], months: number): string {
	const lines: string[] = [];
	lines.push('# Financial Summary');
	lines.push('');
	lines.push(`Generated: ${today()} | Period: ${months} months | Currency: GBP`);
	lines.push('');

	const groupTotals = computeGroupTotals(db, groupIds);

	for (const gid of groupIds) {
		if (!groupTotals.has(gid)) continue;
		lines.push(...renderGroupSection(db, gid, months));
	}

	if (groupIds.length > 1 && groupTotals.size > 1) {
		lines.push(...renderConsolidatedSection(db, groupIds, groupTotals, months));
	}

	lines.push(...renderBalanceSheetSection(db));

	lines.push(...renderMethodology(groupIds));
	lines.push('');

	return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Output handler
// ---------------------------------------------------------------------------

function writeOutput(content: string, outputPath: string | undefined): void {
	if (outputPath) {
		try {
			writeFileSync(outputPath, content, 'utf-8');
			log(`Summary written to ${outputPath}`);
		} catch (e) {
			process.stderr.write(`Error: Failed to write to ${outputPath}: ${e instanceof Error ? e.message : String(e)}\n`);
			process.exit(1);
		}
	} else {
		log(content);
		try {
			const proc = Bun.spawn(['pbcopy'], { stdin: 'pipe' });
			proc.stdin.write(content);
			proc.stdin.end();
		} catch {
			// clipboard copy is best-effort
		}
	}
}

// ---------------------------------------------------------------------------
// Command definition
// ---------------------------------------------------------------------------

export const reportSummaryCommand = defineToolCommand(
	{
		name: 'report.summary',
		command: 'fin report summary',
		category: 'report',
		outputFields: ['generatedAt', 'periodMonths', 'currency', 'groups', 'consolidated', 'balanceSheet'],
		idempotent: true,
		rateLimit: null,
		example: 'fin report summary --months=12 --group=personal --json',
	},
	{
		meta: {
			name: 'summary',
			description: 'Comprehensive financial summary',
		},
		args: {
			months: { type: 'string' as const, description: 'Cashflow history depth', default: '12' },
			group: { type: 'string' as const, description: 'Filter to single group' },
			output: { type: 'string' as const, description: 'Write Markdown to file (default: stdout + clipboard)' },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
			format: { type: 'string' as const, description: 'Output format: table, json, tsv', default: 'table' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				const months = Number.parseInt(args.months ?? '12', 10);
				const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const groupIds = resolveGroupIds(args.group, jsonMode, start);

				if (jsonMode) {
					const data = buildSummaryJson(db, groupIds, months);
					ok('report.summary', data, start);
				}

				const content = buildSummaryMarkdown(db, groupIds, months);
				writeOutput(content, args.output);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('report.summary', 'DB_ERROR', `Failed to generate summary: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
