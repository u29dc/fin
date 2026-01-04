/**
 * transactions - Query transactions with filters.
 */

import { getGroupChartAccountIds, getTransactions, isGroupId } from 'core';

import { getOption, getOptionAsNumberOrDefault, parseArgs, validateGroupId } from '../args';
import { getReadonlyDb } from '../db';
import { formatAmount, formatCount, formatDate } from '../format';
import { type Column, parseFormat, renderOutput } from '../output';

type TransactionRow = {
	date: string;
	account: string;
	amount: number;
	description: string;
};

const COLUMNS: Column<TransactionRow>[] = [
	{ key: 'date', label: 'Date', minWidth: 10, format: (v) => formatDate(v as string) },
	{ key: 'account', label: 'Account', minWidth: 24 },
	{ key: 'amount', label: 'Amount', align: 'right', minWidth: 12, format: (v) => formatAmount(v as number) },
	{ key: 'description', label: 'Description', minWidth: 30, maxWidth: 40 },
];

export function runTransactions(args: string[]): void {
	const parsed = parseArgs(args);
	const format = parseFormat(getOption(parsed, 'format'));
	const chartAccountId = getOption(parsed, 'account');
	const groupId = getOption(parsed, 'group');
	const from = getOption(parsed, 'from');
	const to = getOption(parsed, 'to');
	const limit = getOptionAsNumberOrDefault(parsed, 'limit', 50);

	validateGroupId(groupId, 'transactions');

	// Determine account filter
	let chartAccountIds: string[] | undefined;
	if (groupId && isGroupId(groupId)) {
		chartAccountIds = getGroupChartAccountIds(groupId);
	} else if (chartAccountId) {
		chartAccountIds = [chartAccountId];
	}

	const db = getReadonlyDb(parsed);
	const options: { chartAccountIds?: string[]; from?: string; to?: string; limit: number } = { limit };
	if (chartAccountIds) options.chartAccountIds = chartAccountIds;
	if (from) options.from = from;
	if (to) options.to = to;
	const transactions = getTransactions(db, options);

	const rows: TransactionRow[] = transactions.map((t) => ({
		date: t.postedAt,
		account: t.chartAccountId,
		amount: t.amountMinor,
		description: t.cleanDescription || t.rawDescription,
	}));

	const summaryText = formatCount(rows.length, 'transaction');
	renderOutput(rows, COLUMNS, format, summaryText);
}
