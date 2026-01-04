import type { Database } from 'bun:sqlite';

import type { BalanceSheet, CategoryBreakdown, ExpenseNode, JournalEntryWithPostings, MonthlyCashflow, Posting } from '../types/ledger';

// ============================================
// LEDGER TYPES (matching groups.ts types)
// ============================================

export type LedgerDailyBalancePoint = {
	date: string;
	balanceMinor: number;
};

export type LedgerLatestBalance = {
	accountId: string;
	date: string | null;
	balanceMinor: number | null;
};

export type LedgerContributionPoint = {
	date: string;
	contributionsMinor: number;
};

export type LedgerMonthlyCashflowPoint = {
	month: string;
	incomeMinor: number;
	expenseMinor: number;
	netMinor: number;
	savingsRatePct: number | null;
	rollingMedianExpenseMinor: number | null;
	expenseDeviationRatio: number | null;
};

export type LedgerCategoryMonthlyMedianPoint = {
	accountId: string;
	categoryName: string;
	monthlyMedianMinor: number;
	monthCount: number;
};

export type LedgerBalanceSeriesOptions = {
	from?: string;
	to?: string;
	limit?: number;
};

export type LedgerCashflowSeriesOptions = {
	from?: string;
	to?: string;
	limit?: number;
};

type BalanceRow = {
	balance: number;
};

type AccountTypeRow = {
	account_type: string;
	total: number;
};

type CashflowRow = {
	month: string;
	income_minor: number;
	expense_minor: number;
};

type CategoryRow = {
	account_id: string;
	category_name: string;
	total_minor: number;
	transaction_count: number;
};

type JournalEntryRow = {
	id: string;
	posted_at: string;
	description: string;
	raw_description: string | null;
	clean_description: string | null;
	counterparty: string | null;
	source_file: string | null;
	created_at: string;
	updated_at: string;
};

type PostingRow = {
	id: string;
	journal_entry_id: string;
	account_id: string;
	amount_minor: number;
	currency: string;
	memo: string | null;
	provider_txn_id: string | null;
	provider_balance_minor: number | null;
	created_at: string;
};

export function getAccountBalance(db: Database, accountId: string, asOf?: string): number {
	const params: string[] = [accountId, `${accountId}:%`];
	let dateFilter = '';

	if (asOf) {
		dateFilter = 'AND je.posted_at <= ?';
		params.push(asOf);
	}

	const result = db
		.query<BalanceRow, string[]>(
			`
		SELECT COALESCE(SUM(p.amount_minor), 0) as balance
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		WHERE (p.account_id = ? OR p.account_id LIKE ?)
		${dateFilter}
	`,
		)
		.get(...params);

	return result?.balance ?? 0;
}

export function getMonthlyCashflow(db: Database, startDate?: string, endDate?: string): MonthlyCashflow[] {
	const params: string[] = [];
	const conditions: string[] = [];

	if (startDate) {
		conditions.push('je.posted_at >= ?');
		params.push(startDate);
	}
	if (endDate) {
		conditions.push('je.posted_at <= ?');
		params.push(endDate);
	}

	const whereClause = conditions.length > 0 ? `AND ${conditions.join(' AND ')}` : '';

	const rows = db
		.query<CashflowRow, string[]>(
			`
		SELECT
			strftime('%Y-%m', je.posted_at) AS month,
			SUM(CASE WHEN coa.account_type = 'income' THEN -p.amount_minor ELSE 0 END) AS income_minor,
			SUM(CASE WHEN coa.account_type = 'expense' THEN p.amount_minor ELSE 0 END) AS expense_minor
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		JOIN chart_of_accounts coa ON p.account_id = coa.id
		WHERE coa.account_type IN ('income', 'expense')
		${whereClause}
		GROUP BY month
		ORDER BY month ASC
	`,
		)
		.all(...params);

	return rows.map((row) => ({
		month: row.month,
		incomeMinor: row.income_minor,
		expenseMinor: row.expense_minor,
	}));
}

export function getExpensesByCategory(db: Database, months = 3): CategoryBreakdown[] {
	const rows = db
		.query<CategoryRow, [number]>(
			`
		SELECT
			p.account_id,
			coa.name as category_name,
			SUM(p.amount_minor) as total_minor,
			COUNT(*) as transaction_count
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		JOIN chart_of_accounts coa ON p.account_id = coa.id
		WHERE coa.account_type = 'expense'
			AND je.posted_at >= date('now', '-' || ? || ' months')
		GROUP BY p.account_id
		ORDER BY total_minor DESC
	`,
		)
		.all(months);

	return rows.map((row) => ({
		accountId: row.account_id,
		categoryName: row.category_name,
		totalMinor: row.total_minor,
		transactionCount: row.transaction_count,
	}));
}

export function getExpenseHierarchy(db: Database, months = 3): ExpenseNode[] {
	type TotalRow = {
		account_id: string;
		total_minor: number;
	};

	const totals = db
		.query<TotalRow, [number]>(
			`
		SELECT
			p.account_id,
			SUM(p.amount_minor) as total_minor
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		JOIN chart_of_accounts coa ON p.account_id = coa.id
		WHERE coa.account_type = 'expense'
			AND je.posted_at >= date('now', '-' || ? || ' months')
		GROUP BY p.account_id
	`,
		)
		.all(months);

	// Build map of account_id to total
	const totalMap = new Map(totals.map((t) => [t.account_id, t.total_minor]));

	// Get all expense accounts
	type AccountRow = {
		id: string;
		name: string;
		parent_id: string | null;
		is_placeholder: number;
	};

	const accounts = db
		.query<AccountRow, []>(
			`
		SELECT id, name, parent_id, is_placeholder
		FROM chart_of_accounts
		WHERE account_type = 'expense'
		ORDER BY id
	`,
		)
		.all();

	// Build tree
	const nodeMap = new Map<string, ExpenseNode>();
	const rootNodes: ExpenseNode[] = [];

	// Create nodes
	for (const acc of accounts) {
		nodeMap.set(acc.id, {
			accountId: acc.id,
			name: acc.name,
			totalMinor: totalMap.get(acc.id) ?? 0,
			children: [],
		});
	}

	// Build parent-child relationships
	for (const acc of accounts) {
		const node = nodeMap.get(acc.id);
		if (!node) continue;

		if (acc.parent_id && nodeMap.has(acc.parent_id)) {
			nodeMap.get(acc.parent_id)?.children.push(node);
		} else if (acc.id === 'Expenses') {
			rootNodes.push(node);
		}
	}

	// Calculate totals for placeholder accounts (sum of children)
	function calculateTotal(node: ExpenseNode): number {
		if (node.children.length === 0) {
			return node.totalMinor;
		}
		const childTotal = node.children.reduce((sum, child) => sum + calculateTotal(child), 0);
		node.totalMinor = childTotal;
		return childTotal;
	}

	for (const root of rootNodes) {
		calculateTotal(root);
	}

	return rootNodes;
}

export function getBalanceSheet(db: Database, asOf?: string): BalanceSheet {
	const dateFilter = asOf ? 'WHERE je.posted_at <= ?' : '';
	const params = asOf ? [asOf] : [];

	const rows = db
		.query<AccountTypeRow, string[]>(
			`
		SELECT
			coa.account_type,
			SUM(p.amount_minor) as total
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		JOIN chart_of_accounts coa ON p.account_id = coa.id
		${dateFilter}
		GROUP BY coa.account_type
	`,
		)
		.all(...params);

	const byType: Record<string, number> = {};
	for (const row of rows) {
		byType[row.account_type] = row.total;
	}

	const assets = byType['asset'] ?? 0;
	const liabilities = byType['liability'] ?? 0;
	const equity = byType['equity'] ?? 0;
	const income = byType['income'] ?? 0;
	const expenses = byType['expense'] ?? 0;

	return {
		assets,
		liabilities: -liabilities,
		equity: -equity,
		income: -income,
		expenses,
		netWorth: assets + liabilities,
		netIncome: -income - expenses,
	};
}

function loadPostingsForEntry(db: Database, journalEntryId: string): Posting[] {
	const rows = db.query<PostingRow, [string]>(`SELECT * FROM postings WHERE journal_entry_id = ? ORDER BY id`).all(journalEntryId);

	return rows.map((row) => ({
		id: row.id,
		journalEntryId: row.journal_entry_id,
		accountId: row.account_id,
		amountMinor: row.amount_minor,
		currency: row.currency,
		memo: row.memo,
		providerTxnId: row.provider_txn_id,
		providerBalanceMinor: row.provider_balance_minor,
		createdAt: row.created_at,
	}));
}

export type GetJournalEntriesOptions = {
	accountId?: string;
	startDate?: string;
	endDate?: string;
	limit?: number;
	offset?: number;
};

export function getJournalEntries(db: Database, options: GetJournalEntriesOptions = {}): JournalEntryWithPostings[] {
	const conditions: string[] = [];
	const params: (string | number)[] = [];

	if (options.accountId) {
		conditions.push(`je.id IN (
			SELECT journal_entry_id FROM postings WHERE account_id = ? OR account_id LIKE ? || ':%'
		)`);
		params.push(options.accountId, options.accountId);
	}

	if (options.startDate) {
		conditions.push('je.posted_at >= ?');
		params.push(options.startDate);
	}

	if (options.endDate) {
		conditions.push('je.posted_at <= ?');
		params.push(options.endDate);
	}

	const whereClause = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
	const limit = options.limit ?? 100;
	const offset = options.offset ?? 0;

	params.push(limit, offset);

	const entries = db
		.query<JournalEntryRow, (string | number)[]>(
			`
		SELECT je.*
		FROM journal_entries je
		${whereClause}
		ORDER BY je.posted_at DESC
		LIMIT ? OFFSET ?
	`,
		)
		.all(...params);

	return entries.map((entry) => ({
		id: entry.id,
		postedAt: entry.posted_at,
		description: entry.description,
		rawDescription: entry.raw_description,
		cleanDescription: entry.clean_description,
		counterparty: entry.counterparty,
		sourceFile: entry.source_file,
		createdAt: entry.created_at,
		updatedAt: entry.updated_at,
		postings: loadPostingsForEntry(db, entry.id),
	}));
}

export function getJournalEntryCount(db: Database, accountId?: string): number {
	if (accountId) {
		const result = db
			.query<{ count: number }, [string, string]>(
				`
			SELECT COUNT(DISTINCT je.id) as count
			FROM journal_entries je
			JOIN postings p ON p.journal_entry_id = je.id
			WHERE p.account_id = ? OR p.account_id LIKE ? || ':%'
		`,
			)
			.get(accountId, accountId);
		return result?.count ?? 0;
	}

	const result = db.query<{ count: number }, []>(`SELECT COUNT(*) as count FROM journal_entries`).get();
	return result?.count ?? 0;
}

export function getChartOfAccounts(db: Database): Array<{
	id: string;
	name: string;
	accountType: string;
	parentId: string | null;
	isPlaceholder: boolean;
	balance: number;
}> {
	type Row = {
		id: string;
		name: string;
		account_type: string;
		parent_id: string | null;
		is_placeholder: number;
		balance: number | null;
	};

	const rows = db
		.query<Row, []>(
			`
		SELECT
			coa.id,
			coa.name,
			coa.account_type,
			coa.parent_id,
			coa.is_placeholder,
			COALESCE(SUM(p.amount_minor), 0) as balance
		FROM chart_of_accounts coa
		LEFT JOIN postings p ON p.account_id = coa.id
		GROUP BY coa.id
		ORDER BY coa.id
	`,
		)
		.all();

	return rows.map((row) => ({
		id: row.id,
		name: row.name,
		accountType: row.account_type,
		parentId: row.parent_id,
		isPlaceholder: row.is_placeholder === 1,
		balance: row.balance ?? 0,
	}));
}

// ============================================
// LEDGER QUERIES - Replacements for legacy groups.ts
// ============================================

type DailyPostingRow = {
	date: string;
	daily_amount: number;
};

/**
 * Get daily balance series for a chart account (e.g., 'Assets:Personal:Monzo').
 * Computes running balance from postings instead of reading from daily_balances table.
 */
export function getLedgerDailyBalanceSeries(db: Database, chartAccountId: string, options: LedgerBalanceSeriesOptions = {}): LedgerDailyBalancePoint[] {
	const { from, to, limit = 10_000 } = options;

	const conditions: string[] = ['(p.account_id = ? OR p.account_id LIKE ?)'];
	const params: (string | number)[] = [chartAccountId, `${chartAccountId}:%`];

	if (from) {
		conditions.push('DATE(je.posted_at) >= ?');
		params.push(from);
	}
	if (to) {
		conditions.push('DATE(je.posted_at) <= ?');
		params.push(to);
	}

	const sql = `
		SELECT
			DATE(je.posted_at) AS date,
			SUM(p.amount_minor) AS daily_amount
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		WHERE ${conditions.join(' AND ')}
		GROUP BY DATE(je.posted_at)
		ORDER BY date ASC
		LIMIT ?
	`;
	params.push(limit);

	const rows = db.query<DailyPostingRow, (string | number)[]>(sql).all(...params);

	// Compute running balance
	let runningBalance = 0;

	// If we have a 'from' filter, we need to get the starting balance
	if (from) {
		const startBalance = db
			.query<{ balance: number }, [string, string, string]>(
				`
			SELECT COALESCE(SUM(p.amount_minor), 0) as balance
			FROM postings p
			JOIN journal_entries je ON p.journal_entry_id = je.id
			WHERE (p.account_id = ? OR p.account_id LIKE ?)
				AND DATE(je.posted_at) < ?
		`,
			)
			.get(chartAccountId, `${chartAccountId}:%`, from);
		runningBalance = startBalance?.balance ?? 0;
	}

	const result: LedgerDailyBalancePoint[] = [];
	for (const row of rows) {
		runningBalance += row.daily_amount;
		result.push({ date: row.date, balanceMinor: runningBalance });
	}

	return result;
}

type AccountMappingSQL = {
	caseClause: string;
	orClause: string;
	caseParams: (string | number)[];
	orParams: (string | number)[];
};

function buildAccountMappingSQL(chartAccountIds: string[]): AccountMappingSQL {
	const caseExpressions: string[] = [];
	const caseParams: (string | number)[] = [];
	const orConditions: string[] = [];
	const orParams: (string | number)[] = [];

	for (const accountId of chartAccountIds) {
		caseExpressions.push(`WHEN p.account_id = ? OR p.account_id LIKE ? THEN ?`);
		caseParams.push(accountId, `${accountId}:%`, accountId);
		orConditions.push('(p.account_id = ? OR p.account_id LIKE ?)');
		orParams.push(accountId, `${accountId}:%`);
	}

	return {
		caseClause: `CASE ${caseExpressions.join(' ')} END`,
		orClause: orConditions.join(' OR '),
		caseParams,
		orParams,
	};
}

function fetchStartingBalances(db: Database, mapping: AccountMappingSQL, fromDate: string): Map<string, number> {
	type StartBalanceRow = { chart_account_id: string; balance: number };
	const params = [...mapping.caseParams, ...mapping.orParams, fromDate];
	const rows = db
		.query<StartBalanceRow, (string | number)[]>(
			`SELECT ${mapping.caseClause} as chart_account_id, COALESCE(SUM(p.amount_minor), 0) as balance
			FROM postings p JOIN journal_entries je ON p.journal_entry_id = je.id
			WHERE (${mapping.orClause}) AND DATE(je.posted_at) < ? GROUP BY chart_account_id`,
		)
		.all(...params);

	return new Map(rows.map((r) => [r.chart_account_id, r.balance]));
}

/**
 * Get daily balance series for multiple chart accounts in a single query.
 * Returns a record mapping each account ID to its balance series.
 */
export function getLedgerAllAccountsDailyBalanceSeries(db: Database, chartAccountIds: string[], options: LedgerBalanceSeriesOptions = {}): Record<string, LedgerDailyBalancePoint[]> {
	if (chartAccountIds.length === 0) return {};

	const { from, to, limit = 10_000 } = options;
	const mapping = buildAccountMappingSQL(chartAccountIds);

	// Build query params and conditions
	const params: (string | number)[] = [...mapping.caseParams, ...mapping.orParams];
	const conditions = [`(${mapping.orClause})`];
	if (from) {
		conditions.push('DATE(je.posted_at) >= ?');
		params.push(from);
	}
	if (to) {
		conditions.push('DATE(je.posted_at) <= ?');
		params.push(to);
	}

	type BatchedDailyRow = { chart_account_id: string; date: string; daily_amount: number };
	const sql = `SELECT ${mapping.caseClause} as chart_account_id, DATE(je.posted_at) AS date, SUM(p.amount_minor) AS daily_amount
		FROM postings p JOIN journal_entries je ON p.journal_entry_id = je.id
		WHERE ${conditions.join(' AND ')} GROUP BY chart_account_id, DATE(je.posted_at) ORDER BY chart_account_id, date ASC`;
	const rows = db.query<BatchedDailyRow, (string | number)[]>(sql).all(...params);

	// Get starting balances if 'from' filter is used
	const startingBalances = from ? fetchStartingBalances(db, mapping, from) : new Map<string, number>();

	// Initialize result and running balances
	const result: Record<string, LedgerDailyBalancePoint[]> = {};
	const runningBalances = new Map<string, number>();
	for (const accountId of chartAccountIds) {
		result[accountId] = [];
		runningBalances.set(accountId, startingBalances.get(accountId) ?? 0);
	}

	// Process rows and compute running balances
	for (const row of rows) {
		const newBalance = (runningBalances.get(row.chart_account_id) ?? 0) + row.daily_amount;
		runningBalances.set(row.chart_account_id, newBalance);
		result[row.chart_account_id]?.push({ date: row.date, balanceMinor: newBalance });
	}

	// Apply limit per account if needed
	if (limit < 10_000) {
		for (const accountId of chartAccountIds) {
			const series = result[accountId];
			if (series && series.length > limit) result[accountId] = series.slice(0, limit);
		}
	}

	return result;
}

/**
 * Get latest balances for multiple chart accounts.
 * Optimized to use a single query instead of N separate queries.
 */
export function getLedgerLatestBalances(db: Database, chartAccountIds: string[]): LedgerLatestBalance[] {
	if (chartAccountIds.length === 0) {
		return [];
	}

	// Build CASE expressions to map postings to their parent chart account
	const caseExpressions: string[] = [];
	const params: string[] = [];

	for (const accountId of chartAccountIds) {
		caseExpressions.push(`WHEN p.account_id = ? OR p.account_id LIKE ? THEN ?`);
		params.push(accountId, `${accountId}:%`, accountId);
	}

	// Build OR conditions for filtering
	const orConditions = chartAccountIds.map(() => '(p.account_id = ? OR p.account_id LIKE ?)').join(' OR ');
	for (const accountId of chartAccountIds) {
		params.push(accountId, `${accountId}:%`);
	}

	type BatchedRow = {
		chart_account_id: string;
		max_date: string | null;
		balance: number;
	};

	const rows = db
		.query<BatchedRow, string[]>(
			`
		SELECT
			CASE ${caseExpressions.join(' ')} END as chart_account_id,
			MAX(DATE(je.posted_at)) as max_date,
			COALESCE(SUM(p.amount_minor), 0) as balance
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		WHERE ${orConditions}
		GROUP BY chart_account_id
	`,
		)
		.all(...params);

	// Build result map for quick lookup
	const resultMap = new Map<string, { date: string | null; balance: number }>();
	for (const row of rows) {
		resultMap.set(row.chart_account_id, {
			date: row.max_date,
			balance: row.balance,
		});
	}

	// Return results in the same order as input, with defaults for missing accounts
	return chartAccountIds.map((accountId) => {
		const data = resultMap.get(accountId);
		return {
			accountId,
			date: data?.date ?? null,
			balanceMinor: data?.balance ?? null,
		};
	});
}

/**
 * Get daily balance series for multiple chart accounts, aggregated.
 */
export function getLedgerAccountsDailyBalanceSeries(db: Database, chartAccountIds: string[], options: LedgerBalanceSeriesOptions = {}): LedgerDailyBalancePoint[] {
	if (chartAccountIds.length === 0) {
		return [];
	}

	const { from, to, limit = 10_000 } = options;

	// Build OR conditions for account matching
	const orConditions: string[] = [];
	const matchParams: string[] = [];
	for (const accountId of chartAccountIds) {
		orConditions.push('(p.account_id = ? OR p.account_id LIKE ?)');
		matchParams.push(accountId, `${accountId}:%`);
	}

	const conditions: string[] = [`(${orConditions.join(' OR ')})`];
	const params: (string | number)[] = [...matchParams];

	if (from) {
		conditions.push('DATE(je.posted_at) >= ?');
		params.push(from);
	}
	if (to) {
		conditions.push('DATE(je.posted_at) <= ?');
		params.push(to);
	}

	const sql = `
		SELECT
			DATE(je.posted_at) AS date,
			SUM(p.amount_minor) AS daily_amount
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		WHERE ${conditions.join(' AND ')}
		GROUP BY DATE(je.posted_at)
		ORDER BY date ASC
		LIMIT ?
	`;
	params.push(limit);

	const rows = db.query<DailyPostingRow, (string | number)[]>(sql).all(...params);

	// Compute running balance
	let runningBalance = 0;

	// Get starting balance if we have a 'from' filter
	if (from) {
		const startConditions = [`(${orConditions.join(' OR ')})`, 'DATE(je.posted_at) < ?'];
		const startParams = [...matchParams, from];

		const startBalance = db
			.query<{ balance: number }, string[]>(
				`
			SELECT COALESCE(SUM(p.amount_minor), 0) as balance
			FROM postings p
			JOIN journal_entries je ON p.journal_entry_id = je.id
			WHERE ${startConditions.join(' AND ')}
		`,
			)
			.get(...startParams);
		runningBalance = startBalance?.balance ?? 0;
	}

	const result: LedgerDailyBalancePoint[] = [];
	for (const row of rows) {
		runningBalance += row.daily_amount;
		result.push({ date: row.date, balanceMinor: runningBalance });
	}

	return result;
}

type MonthlyCashflowFullRow = {
	month: string;
	income_minor: number;
	expense_minor: number;
};

/**
 * Get monthly cashflow series with rolling statistics.
 * Computes from income/expense postings for specified chart accounts.
 */
export function getLedgerMonthlyCashflowSeries(db: Database, chartAccountIds: string[], options: LedgerCashflowSeriesOptions = {}): LedgerMonthlyCashflowPoint[] {
	if (chartAccountIds.length === 0) {
		return [];
	}

	const { from, to, limit = 120 } = options;

	// Build OR conditions for account matching (asset accounts)
	const orConditions: string[] = [];
	const matchParams: string[] = [];
	for (const accountId of chartAccountIds) {
		orConditions.push('(p.account_id = ? OR p.account_id LIKE ?)');
		matchParams.push(accountId, `${accountId}:%`);
	}

	const conditions: string[] = [];
	const params: string[] = [...matchParams];

	if (from) {
		conditions.push('je.posted_at >= ?');
		params.push(`${from}T00:00:00`);
	}
	if (to) {
		conditions.push('je.posted_at <= ?');
		params.push(`${to}T23:59:59`);
	}

	const whereClause = conditions.length > 0 ? `AND ${conditions.join(' AND ')}` : '';

	// For cashflow, we need entries that have a posting to one of our asset accounts
	// Income = positive postings to asset account (counter-posting is to Income:*)
	// Expense = negative postings to asset account (counter-posting is to Expenses:*)
	// We exclude transfers (both postings are to asset accounts)
	const sql = `
		SELECT
			strftime('%Y-%m', je.posted_at) AS month,
			SUM(CASE WHEN p.amount_minor > 0 AND NOT is_transfer THEN p.amount_minor ELSE 0 END) AS income_minor,
			SUM(CASE WHEN p.amount_minor < 0 AND NOT is_transfer THEN -p.amount_minor ELSE 0 END) AS expense_minor
		FROM (
			SELECT
				p.journal_entry_id,
				p.amount_minor,
				EXISTS (
					SELECT 1 FROM postings p2
					WHERE p2.journal_entry_id = p.journal_entry_id
						AND p2.id != p.id
						AND p2.account_id LIKE 'Assets:%'
				) AS is_transfer
			FROM postings p
			WHERE (${orConditions.join(' OR ')})
		) p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		WHERE 1=1 ${whereClause}
		GROUP BY month
		ORDER BY month ASC
		LIMIT ?
	`;
	params.push(String(limit));

	const rows = db.query<MonthlyCashflowFullRow, string[]>(sql).all(...params);

	// First pass: create base points
	const basePoints = rows.map((row) => {
		const netMinor = row.income_minor - row.expense_minor;
		const savingsRatePct = row.income_minor > 0 ? Math.round((netMinor / row.income_minor) * 1000) / 10 : null;
		return {
			month: row.month,
			incomeMinor: row.income_minor,
			expenseMinor: row.expense_minor,
			netMinor,
			savingsRatePct,
		};
	});

	// Second pass: calculate rolling 6-month median expense and deviation
	const ROLLING_WINDOW = 6;
	return basePoints.map((point, i) => {
		const start = Math.max(0, i - ROLLING_WINDOW);
		const prevExpenses = basePoints.slice(start, i).map((p) => p.expenseMinor);

		let rollingMedianExpenseMinor: number | null = null;
		let expenseDeviationRatio: number | null = null;

		if (prevExpenses.length >= 3) {
			const sorted = [...prevExpenses].sort((a, b) => a - b);
			const mid = Math.floor(sorted.length / 2);
			rollingMedianExpenseMinor = sorted.length % 2 === 1 ? (sorted[mid] ?? 0) : Math.round(((sorted[mid - 1] ?? 0) + (sorted[mid] ?? 0)) / 2);

			if (rollingMedianExpenseMinor > 0) {
				expenseDeviationRatio = Math.round((point.expenseMinor / rollingMedianExpenseMinor) * 100) / 100;
			}
		}

		return {
			...point,
			rollingMedianExpenseMinor,
			expenseDeviationRatio,
		};
	});
}

type ContributionPostingRow = {
	posted_at: string;
	amount_minor: number;
};

/**
 * Get cumulative contribution series for an account (e.g., for tracking ISA deposits).
 */
export function getLedgerCumulativeContributionSeries(db: Database, chartAccountId: string, options: LedgerBalanceSeriesOptions = {}): LedgerContributionPoint[] {
	const { from, to, limit = 50_000 } = options;

	const conditions: string[] = ['(p.account_id = ? OR p.account_id LIKE ?)'];
	const params: (string | number)[] = [chartAccountId, `${chartAccountId}:%`];

	if (from) {
		conditions.push('je.posted_at >= ?');
		params.push(`${from}T00:00:00`);
	}
	if (to) {
		conditions.push('je.posted_at <= ?');
		params.push(`${to}T23:59:59`);
	}

	const sql = `
		SELECT je.posted_at, p.amount_minor
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		WHERE ${conditions.join(' AND ')}
		ORDER BY je.posted_at ASC
		LIMIT ?
	`;
	params.push(limit);

	const rows = db.query<ContributionPostingRow, (string | number)[]>(sql).all(...params);

	let running = 0;
	const result: LedgerContributionPoint[] = [];

	for (const row of rows) {
		running += row.amount_minor;
		const date = row.posted_at.slice(0, 10);
		const last = result[result.length - 1];
		if (last && last.date === date) {
			last.contributionsMinor = running;
		} else {
			result.push({ date, contributionsMinor: running });
		}
	}

	return result;
}

type CategoryMonthRow = {
	account_id: string;
	category_name: string;
	month: string;
	month_total: number;
};

export type LedgerCategoryMonthlyMedianOptions = {
	months?: number;
	limit?: number;
};

/**
 * Get monthly median expense by category for expense accounts.
 */
export function getLedgerCategoryMonthlyMedian(db: Database, chartAccountIds: string[], options: LedgerCategoryMonthlyMedianOptions = {}): LedgerCategoryMonthlyMedianPoint[] {
	const { months = 6, limit = 10 } = options;

	if (chartAccountIds.length === 0) {
		return [];
	}

	// Build OR conditions for account matching (asset accounts to filter entries)
	const orConditions: string[] = [];
	const matchParams: string[] = [];
	for (const accountId of chartAccountIds) {
		orConditions.push('(asset_posting.account_id = ? OR asset_posting.account_id LIKE ?)');
		matchParams.push(accountId, `${accountId}:%`);
	}

	// Get monthly totals per expense category
	// We want expenses that have a counter-posting to one of our asset accounts
	const sql = `
		SELECT
			p.account_id,
			coa.name as category_name,
			strftime('%Y-%m', je.posted_at) AS month,
			SUM(p.amount_minor) AS month_total
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		JOIN chart_of_accounts coa ON p.account_id = coa.id
		WHERE coa.account_type = 'expense'
			AND je.posted_at >= date('now', '-' || ? || ' months')
			AND EXISTS (
				SELECT 1 FROM postings asset_posting
				WHERE asset_posting.journal_entry_id = p.journal_entry_id
					AND (${orConditions.join(' OR ')})
			)
		GROUP BY p.account_id, strftime('%Y-%m', je.posted_at)
		ORDER BY p.account_id, month
	`;
	const params: (string | number)[] = [months, ...matchParams];

	const rows = db.query<CategoryMonthRow, (string | number)[]>(sql).all(...params);

	// Group by account and calculate median
	const byAccount = new Map<string, { name: string; totals: number[] }>();
	for (const row of rows) {
		if (!byAccount.has(row.account_id)) {
			byAccount.set(row.account_id, { name: row.category_name, totals: [] });
		}
		byAccount.get(row.account_id)?.totals.push(row.month_total);
	}

	// Calculate median for each account
	const results: LedgerCategoryMonthlyMedianPoint[] = [];
	for (const [accountId, data] of byAccount) {
		const sorted = [...data.totals].sort((a, b) => a - b);
		const mid = Math.floor(sorted.length / 2);
		const median = sorted.length % 2 === 1 ? (sorted[mid] ?? 0) : Math.round(((sorted[mid - 1] ?? 0) + (sorted[mid] ?? 0)) / 2);

		results.push({
			accountId,
			categoryName: data.name,
			monthlyMedianMinor: median,
			monthCount: data.totals.length,
		});
	}

	// Sort by median descending and limit
	return results.sort((a, b) => b.monthlyMedianMinor - a.monthlyMedianMinor).slice(0, limit);
}

// ============================================
// GROUP EXPENSE HIERARCHY FOR TREEMAP
// ============================================

export type GroupExpenseHierarchyOptions = {
	months?: number;
};

/**
 * Get expense hierarchy filtered by transactions involving specified chart accounts.
 * This allows filtering expenses by group (e.g., personal vs business expenses).
 */
export function getGroupExpenseHierarchy(db: Database, chartAccountIds: string[], options: GroupExpenseHierarchyOptions = {}): ExpenseNode[] {
	const { months = 3 } = options;

	if (chartAccountIds.length === 0) {
		return [];
	}

	// Build OR conditions for account matching
	const orConditions = chartAccountIds.flatMap(() => ['asset_posting.account_id = ?', 'asset_posting.account_id LIKE ?']);
	const matchParams = chartAccountIds.flatMap((id) => [id, `${id}:%`]);

	type TotalRow = {
		account_id: string;
		total_minor: number;
	};

	const sql = `
		SELECT
			p.account_id,
			SUM(p.amount_minor) as total_minor
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		JOIN chart_of_accounts coa ON p.account_id = coa.id
		WHERE coa.account_type = 'expense'
			AND je.posted_at >= date('now', '-' || ? || ' months')
			AND EXISTS (
				SELECT 1 FROM postings asset_posting
				WHERE asset_posting.journal_entry_id = p.journal_entry_id
					AND (${orConditions.join(' OR ')})
			)
		GROUP BY p.account_id
	`;

	const totals = db.query<TotalRow, (string | number)[]>(sql).all(months, ...matchParams);

	// Build map of account_id to total
	const totalMap = new Map(totals.map((t) => [t.account_id, t.total_minor]));

	// Get all expense accounts that have transactions
	type AccountRow = {
		id: string;
		name: string;
		parent_id: string | null;
		is_placeholder: number;
	};

	const accounts = db
		.query<AccountRow, []>(
			`
		SELECT id, name, parent_id, is_placeholder
		FROM chart_of_accounts
		WHERE account_type = 'expense'
		ORDER BY id
	`,
		)
		.all();

	// Build tree
	const nodeMap = new Map<string, ExpenseNode>();
	const rootNodes: ExpenseNode[] = [];

	// Create nodes
	for (const acc of accounts) {
		nodeMap.set(acc.id, {
			accountId: acc.id,
			name: acc.name,
			totalMinor: totalMap.get(acc.id) ?? 0,
			children: [],
		});
	}

	// Build parent-child relationships
	for (const acc of accounts) {
		const node = nodeMap.get(acc.id);
		if (!node) continue;

		if (acc.parent_id && nodeMap.has(acc.parent_id)) {
			nodeMap.get(acc.parent_id)?.children.push(node);
		} else if (acc.id === 'Expenses') {
			rootNodes.push(node);
		}
	}

	// Calculate totals for placeholder accounts (sum of children)
	function calculateTotal(node: ExpenseNode): number {
		if (node.children.length === 0) {
			return node.totalMinor;
		}
		const childTotal = node.children.reduce((sum, child) => sum + calculateTotal(child), 0);
		node.totalMinor = childTotal;
		return childTotal;
	}

	for (const root of rootNodes) {
		calculateTotal(root);
	}

	// Filter out nodes with zero total
	function pruneZeroNodes(nodes: ExpenseNode[]): ExpenseNode[] {
		return nodes
			.filter((node) => node.totalMinor > 0)
			.map((node) => ({
				...node,
				children: pruneZeroNodes(node.children),
			}));
	}

	return pruneZeroNodes(rootNodes);
}

// ============================================
// GROUP EXPENSE HIERARCHY (MEDIAN-BASED) FOR TREEMAP
// ============================================

type MonthlyTotalRow = {
	account_id: string;
	month: string;
	month_total: number;
};

function calculateMedianMap(monthlyTotals: MonthlyTotalRow[]): Map<string, number> {
	// Group by account
	const byAccount = new Map<string, number[]>();
	for (const row of monthlyTotals) {
		if (!byAccount.has(row.account_id)) {
			byAccount.set(row.account_id, []);
		}
		byAccount.get(row.account_id)?.push(row.month_total);
	}

	// Calculate median for each account
	const medianMap = new Map<string, number>();
	for (const [accountId, totals] of byAccount) {
		const sorted = [...totals].sort((a, b) => a - b);
		const mid = Math.floor(sorted.length / 2);
		const median = sorted.length % 2 === 1 ? (sorted[mid] ?? 0) : Math.round(((sorted[mid - 1] ?? 0) + (sorted[mid] ?? 0)) / 2);
		medianMap.set(accountId, median);
	}
	return medianMap;
}

type MedianAccountRow = {
	id: string;
	name: string;
	parent_id: string | null;
	is_placeholder: number;
};

function buildMedianExpenseTree(accounts: MedianAccountRow[], medianMap: Map<string, number>): ExpenseNode[] {
	const nodeMap = new Map<string, ExpenseNode>();
	const rootNodes: ExpenseNode[] = [];

	// Create nodes
	for (const acc of accounts) {
		nodeMap.set(acc.id, {
			accountId: acc.id,
			name: acc.name,
			totalMinor: medianMap.get(acc.id) ?? 0,
			children: [],
		});
	}

	// Build parent-child relationships
	for (const acc of accounts) {
		const node = nodeMap.get(acc.id);
		if (!node) continue;

		if (acc.parent_id && nodeMap.has(acc.parent_id)) {
			nodeMap.get(acc.parent_id)?.children.push(node);
		} else if (acc.id === 'Expenses') {
			rootNodes.push(node);
		}
	}

	// Calculate totals for placeholder accounts
	function calculateTotal(node: ExpenseNode): number {
		if (node.children.length === 0) return node.totalMinor;
		const childTotal = node.children.reduce((sum, child) => sum + calculateTotal(child), 0);
		node.totalMinor = childTotal;
		return childTotal;
	}

	for (const root of rootNodes) {
		calculateTotal(root);
	}

	// Filter out nodes with zero total
	function pruneZeroNodes(nodes: ExpenseNode[]): ExpenseNode[] {
		return nodes.filter((node) => node.totalMinor > 0).map((node) => ({ ...node, children: pruneZeroNodes(node.children) }));
	}

	return pruneZeroNodes(rootNodes);
}

/**
 * Get expense hierarchy with monthly median values for stable representation.
 * Uses median of monthly totals over the specified period.
 */
export function getGroupExpenseHierarchyMedian(db: Database, chartAccountIds: string[], options: GroupExpenseHierarchyOptions = {}): ExpenseNode[] {
	const { months = 6 } = options;

	if (chartAccountIds.length === 0) {
		return [];
	}

	// Build OR conditions for account matching
	const orConditions = chartAccountIds.flatMap(() => ['asset_posting.account_id = ?', 'asset_posting.account_id LIKE ?']);
	const matchParams = chartAccountIds.flatMap((id) => [id, `${id}:%`]);

	const sql = `
		SELECT
			p.account_id,
			strftime('%Y-%m', je.posted_at) AS month,
			SUM(p.amount_minor) as month_total
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		JOIN chart_of_accounts coa ON p.account_id = coa.id
		WHERE coa.account_type = 'expense'
			AND je.posted_at >= date('now', '-' || ? || ' months')
			AND EXISTS (
				SELECT 1 FROM postings asset_posting
				WHERE asset_posting.journal_entry_id = p.journal_entry_id
					AND (${orConditions.join(' OR ')})
			)
		GROUP BY p.account_id, strftime('%Y-%m', je.posted_at)
	`;

	const monthlyTotals = db.query<MonthlyTotalRow, (string | number)[]>(sql).all(months, ...matchParams);
	const medianMap = calculateMedianMap(monthlyTotals);

	const accounts = db
		.query<MedianAccountRow, []>(
			`
		SELECT id, name, parent_id, is_placeholder
		FROM chart_of_accounts
		WHERE account_type = 'expense'
		ORDER BY id
	`,
		)
		.all();

	return buildMedianExpenseTree(accounts, medianMap);
}

// ============================================
// CASH FLOW DATA FOR SANKEY DIAGRAM
// ============================================

export type SankeyFlowData = {
	nodes: Array<{ name: string; category: 'income' | 'asset' | 'expense' }>;
	links: Array<{ source: string; target: string; value: number }>;
};

export type CashFlowOptions = {
	months?: number;
};

type SankeyNodeMap = Map<string, { name: string; category: 'income' | 'asset' | 'expense' }>;

type FlowRow = {
	source_id: string;
	source_name: string;
	target_id: string;
	target_name: string;
	flow_amount: number;
};

function addFlowNodesToMap(flows: FlowRow[], nodeMap: SankeyNodeMap, sourceCategory: 'income' | 'asset' | 'expense', targetCategory: 'income' | 'asset' | 'expense'): void {
	for (const flow of flows) {
		if (!nodeMap.has(flow.source_id)) {
			nodeMap.set(flow.source_id, { name: flow.source_name, category: sourceCategory });
		}
		if (!nodeMap.has(flow.target_id)) {
			nodeMap.set(flow.target_id, { name: flow.target_name, category: targetCategory });
		}
	}
}

function flowsToLinks(flows: FlowRow[], nodeMap: SankeyNodeMap): Array<{ source: string; target: string; value: number }> {
	return flows.map((flow) => ({
		source: nodeMap.get(flow.source_id)?.name ?? flow.source_id,
		target: nodeMap.get(flow.target_id)?.name ?? flow.target_id,
		value: flow.flow_amount,
	}));
}

/**
 * Get cash flow data for Sankey diagram showing money movement:
 * Income sources -> Asset accounts -> Expense categories
 */
export function getCashFlowData(db: Database, chartAccountIds: string[], options: CashFlowOptions = {}): SankeyFlowData {
	const { months = 3 } = options;

	if (chartAccountIds.length === 0) {
		return { nodes: [], links: [] };
	}

	// Build OR conditions for account matching
	const orConditions = chartAccountIds.flatMap(() => ['p_asset.account_id = ?', 'p_asset.account_id LIKE ?']);
	const matchParams = chartAccountIds.flatMap((id) => [id, `${id}:%`]);

	// Income to Asset flows (income is credit/negative, asset is debit/positive)
	const incomeToAssetSql = `
		SELECT
			p_income.account_id as source_id,
			coa_income.name as source_name,
			p_asset.account_id as target_id,
			coa_asset.name as target_name,
			SUM(p_asset.amount_minor) as flow_amount
		FROM postings p_income
		JOIN postings p_asset ON p_income.journal_entry_id = p_asset.journal_entry_id
		JOIN journal_entries je ON p_income.journal_entry_id = je.id
		JOIN chart_of_accounts coa_income ON p_income.account_id = coa_income.id
		JOIN chart_of_accounts coa_asset ON p_asset.account_id = coa_asset.id
		WHERE coa_income.account_type = 'income'
			AND coa_asset.account_type = 'asset'
			AND p_asset.amount_minor > 0
			AND je.posted_at >= date('now', '-' || ? || ' months')
			AND (${orConditions.join(' OR ')})
		GROUP BY p_income.account_id, p_asset.account_id
		HAVING flow_amount > 0
	`;

	const incomeToAsset = db.query<FlowRow, (string | number)[]>(incomeToAssetSql).all(months, ...matchParams);

	// Asset to Expense flows (expense is debit/positive, asset is credit/negative)
	const assetToExpenseSql = `
		SELECT
			p_asset.account_id as source_id,
			coa_asset.name as source_name,
			p_expense.account_id as target_id,
			coa_expense.name as target_name,
			SUM(p_expense.amount_minor) as flow_amount
		FROM postings p_expense
		JOIN postings p_asset ON p_expense.journal_entry_id = p_asset.journal_entry_id
		JOIN journal_entries je ON p_expense.journal_entry_id = je.id
		JOIN chart_of_accounts coa_expense ON p_expense.account_id = coa_expense.id
		JOIN chart_of_accounts coa_asset ON p_asset.account_id = coa_asset.id
		WHERE coa_expense.account_type = 'expense'
			AND coa_asset.account_type = 'asset'
			AND p_expense.amount_minor > 0
			AND je.posted_at >= date('now', '-' || ? || ' months')
			AND (${orConditions.join(' OR ')})
		GROUP BY p_asset.account_id, p_expense.account_id
		HAVING flow_amount > 0
	`;

	const assetToExpense = db.query<FlowRow, (string | number)[]>(assetToExpenseSql).all(months, ...matchParams);

	// Build nodes map
	const nodeMap: SankeyNodeMap = new Map();
	addFlowNodesToMap(incomeToAsset, nodeMap, 'income', 'asset');
	addFlowNodesToMap(assetToExpense, nodeMap, 'asset', 'expense');

	// Build nodes and links arrays
	const nodes = Array.from(nodeMap.values());
	const links = [...flowsToLinks(incomeToAsset, nodeMap), ...flowsToLinks(assetToExpense, nodeMap)];

	return { nodes, links };
}

type MonthlyFlowRow = {
	source_id: string;
	source_name: string;
	target_id: string;
	target_name: string;
	month: string;
	month_flow: number;
};

function calculateMedianFlows(monthlyFlows: MonthlyFlowRow[]): FlowRow[] {
	// Group by source/target pair
	const byPair = new Map<string, { source_id: string; source_name: string; target_id: string; target_name: string; flows: number[] }>();
	for (const row of monthlyFlows) {
		const key = `${row.source_id}:${row.target_id}`;
		if (!byPair.has(key)) {
			byPair.set(key, {
				source_id: row.source_id,
				source_name: row.source_name,
				target_id: row.target_id,
				target_name: row.target_name,
				flows: [],
			});
		}
		byPair.get(key)?.flows.push(row.month_flow);
	}

	// Calculate median for each pair
	const result: FlowRow[] = [];
	for (const data of byPair.values()) {
		const sorted = [...data.flows].sort((a, b) => a - b);
		const mid = Math.floor(sorted.length / 2);
		const median = sorted.length % 2 === 1 ? (sorted[mid] ?? 0) : Math.round(((sorted[mid - 1] ?? 0) + (sorted[mid] ?? 0)) / 2);
		if (median > 0) {
			result.push({
				source_id: data.source_id,
				source_name: data.source_name,
				target_id: data.target_id,
				target_name: data.target_name,
				flow_amount: median,
			});
		}
	}
	return result;
}

/**
 * Get cash flow data with monthly median values for stable representation.
 * Uses median of monthly flows over the specified period.
 */
export function getCashFlowDataMedian(db: Database, chartAccountIds: string[], options: CashFlowOptions = {}): SankeyFlowData {
	const { months = 6 } = options;

	if (chartAccountIds.length === 0) {
		return { nodes: [], links: [] };
	}

	// Build OR conditions for account matching
	const orConditions = chartAccountIds.flatMap(() => ['p_asset.account_id = ?', 'p_asset.account_id LIKE ?']);
	const matchParams = chartAccountIds.flatMap((id) => [id, `${id}:%`]);

	// Income to Asset monthly flows
	const incomeToAssetSql = `
		SELECT
			p_income.account_id as source_id,
			coa_income.name as source_name,
			p_asset.account_id as target_id,
			coa_asset.name as target_name,
			strftime('%Y-%m', je.posted_at) AS month,
			SUM(p_asset.amount_minor) as month_flow
		FROM postings p_income
		JOIN postings p_asset ON p_income.journal_entry_id = p_asset.journal_entry_id
		JOIN journal_entries je ON p_income.journal_entry_id = je.id
		JOIN chart_of_accounts coa_income ON p_income.account_id = coa_income.id
		JOIN chart_of_accounts coa_asset ON p_asset.account_id = coa_asset.id
		WHERE coa_income.account_type = 'income'
			AND coa_asset.account_type = 'asset'
			AND p_asset.amount_minor > 0
			AND je.posted_at >= date('now', '-' || ? || ' months')
			AND (${orConditions.join(' OR ')})
		GROUP BY p_income.account_id, p_asset.account_id, strftime('%Y-%m', je.posted_at)
		HAVING month_flow > 0
	`;

	const incomeToAssetMonthly = db.query<MonthlyFlowRow, (string | number)[]>(incomeToAssetSql).all(months, ...matchParams);
	const incomeToAsset = calculateMedianFlows(incomeToAssetMonthly);

	// Asset to Expense monthly flows
	const assetToExpenseSql = `
		SELECT
			p_asset.account_id as source_id,
			coa_asset.name as source_name,
			p_expense.account_id as target_id,
			coa_expense.name as target_name,
			strftime('%Y-%m', je.posted_at) AS month,
			SUM(p_expense.amount_minor) as month_flow
		FROM postings p_expense
		JOIN postings p_asset ON p_expense.journal_entry_id = p_asset.journal_entry_id
		JOIN journal_entries je ON p_expense.journal_entry_id = je.id
		JOIN chart_of_accounts coa_expense ON p_expense.account_id = coa_expense.id
		JOIN chart_of_accounts coa_asset ON p_asset.account_id = coa_asset.id
		WHERE coa_expense.account_type = 'expense'
			AND coa_asset.account_type = 'asset'
			AND p_expense.amount_minor > 0
			AND je.posted_at >= date('now', '-' || ? || ' months')
			AND (${orConditions.join(' OR ')})
		GROUP BY p_asset.account_id, p_expense.account_id, strftime('%Y-%m', je.posted_at)
		HAVING month_flow > 0
	`;

	const assetToExpenseMonthly = db.query<MonthlyFlowRow, (string | number)[]>(assetToExpenseSql).all(months, ...matchParams);
	const assetToExpense = calculateMedianFlows(assetToExpenseMonthly);

	// Build nodes map
	const nodeMap: SankeyNodeMap = new Map();
	addFlowNodesToMap(incomeToAsset, nodeMap, 'income', 'asset');
	addFlowNodesToMap(assetToExpense, nodeMap, 'asset', 'expense');

	// Build nodes and links arrays
	const nodes = Array.from(nodeMap.values());
	const links = [...flowsToLinks(incomeToAsset, nodeMap), ...flowsToLinks(assetToExpense, nodeMap)];

	return { nodes, links };
}
