import Papa from 'papaparse';
import { getAccountsByProvider, isConfigInitialized } from '../../config/index';
import { parseAmountMinor } from '../../utils/amount';
import { toIsoLocalDateTime } from '../../utils/datetime';
import type { AssetAccountId, ParsedTransaction, ParseResult } from '../types';
import { type ColumnMapping, getColumnMapping, validateCsvHeaders } from './validation';

type MonzoRow = Record<string, string>;

function getColumnValue(row: MonzoRow, column: string | undefined): string | null {
	if (!column) return null;
	const value = row[column]?.trim();
	return value && value.length > 0 ? value : null;
}

function resolveAmount(row: MonzoRow, cols: ColumnMapping): string {
	return getColumnValue(row, cols.amount) || getColumnValue(row, 'Money In') || getColumnValue(row, 'Money Out') || '';
}

function resolveDescription(row: MonzoRow, cols: ColumnMapping): { rawDescription: string; counterparty: string | null } {
	const name = getColumnValue(row, cols.name);
	const description = getColumnValue(row, cols.description);
	const rawDescription = description || name || '';
	const counterparty = name;
	return { rawDescription, counterparty };
}

function isMonzoAccount(chartAccountId: string): boolean {
	if (!isConfigInitialized()) {
		return chartAccountId.includes(':Monzo') || chartAccountId.includes(':Savings');
	}
	const monzoAccounts = getAccountsByProvider('monzo').map((a) => a.id);
	return monzoAccounts.includes(chartAccountId);
}

function parseMonzoRow(row: MonzoRow, cols: ColumnMapping, chartAccountId: AssetAccountId, filePath: string): ParsedTransaction | null {
	const datePart = getColumnValue(row, cols.date);
	const timePart = getColumnValue(row, cols.time);

	if (!datePart) return null;
	if (cols.time && !timePart) return null;

	const postedAt = timePart ? toIsoLocalDateTime(datePart, timePart) : `${datePart}T00:00:00`;
	const { rawDescription, counterparty } = resolveDescription(row, cols);
	const balanceRaw = getColumnValue(row, cols.balance);

	return {
		chartAccountId,
		postedAt,
		amountMinor: parseAmountMinor(resolveAmount(row, cols)),
		currency: getColumnValue(row, 'Currency') || 'GBP',
		rawDescription,
		counterparty,
		providerCategory: getColumnValue(row, cols.category),
		providerTxnId: getColumnValue(row, cols.transactionId),
		balanceMinor: balanceRaw ? parseAmountMinor(balanceRaw) : null,
		sourceFile: filePath,
	};
}

export async function parseMonzoCsv(filePath: string, chartAccountId: AssetAccountId): Promise<ParseResult> {
	if (!isMonzoAccount(chartAccountId)) {
		throw new Error(`Account "${chartAccountId}" is not configured as a Monzo account for file: ${filePath}`);
	}

	// Get column mapping from config
	const cols = getColumnMapping('monzo');

	const text = await Bun.file(filePath).text();
	const result = Papa.parse<MonzoRow>(text, {
		header: true,
		skipEmptyLines: true,
	});

	if (result.errors.length > 0) {
		throw new Error(`Monzo CSV parse errors: ${result.errors.map((e) => e.message).join('; ')}`);
	}

	// Validate headers against config-defined required columns
	const headers = result.meta.fields ?? [];
	validateCsvHeaders(headers, 'monzo');

	const transactions: ParsedTransaction[] = [];

	for (const row of result.data) {
		const txn = parseMonzoRow(row, cols, chartAccountId, filePath);
		if (txn) {
			transactions.push(txn);
		}
	}

	const hasBalances = transactions.some((txn) => txn.balanceMinor !== null);

	return { chartAccountId, transactions, hasBalances };
}
