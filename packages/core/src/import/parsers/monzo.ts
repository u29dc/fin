import Papa from 'papaparse';
import { getAccountsByProvider, isConfigInitialized } from '../../config/index';
import { parseAmountMinor } from '../../utils/amount';
import { toIsoLocalDateTime } from '../../utils/datetime';
import type { AssetAccountId, ParsedTransaction, ParseResult } from '../types';
import { type ColumnMapping, getColumnMapping, validateCsvHeaders } from './validation';

type MonzoRow = Record<string, string>;

function isMonzoAccount(chartAccountId: string): boolean {
	if (!isConfigInitialized()) {
		return chartAccountId.includes(':Monzo') || chartAccountId.includes(':Savings');
	}
	const monzoAccounts = getAccountsByProvider('monzo').map((a) => a.id);
	return monzoAccounts.includes(chartAccountId);
}

function parseMonzoRow(row: MonzoRow, cols: ColumnMapping, chartAccountId: AssetAccountId, filePath: string): ParsedTransaction | null {
	// Use config-defined column names
	const providerTxnId = (cols.transactionId ? row[cols.transactionId]?.trim() : null) || null;
	const datePart = row[cols.date]?.trim();
	const timePart = cols.time ? row[cols.time]?.trim() : undefined;

	if (!datePart) {
		return null;
	}

	// If time column is configured and required (Monzo), skip rows with missing time
	// If no time column configured (Wise), default to midnight
	if (cols.time && !timePart) {
		return null;
	}

	const postedAt = timePart ? toIsoLocalDateTime(datePart, timePart) : `${datePart}T00:00:00`;

	// Amount column with fallback for Money In/Money Out format
	const amountRaw = row[cols.amount]?.trim() || row['Money In']?.trim() || row['Money Out']?.trim() || '';
	const amountMinor = parseAmountMinor(amountRaw);
	const currency = row['Currency']?.trim() || 'GBP';

	const name = cols.name ? row[cols.name]?.trim() : undefined;
	const description = row[cols.description]?.trim();
	const rawDescription = (description && description.length > 0 ? description : name) || '';
	const counterparty = name && name.length > 0 ? name : null;
	const providerCategory = (cols.category ? row[cols.category]?.trim() : null) || null;

	const balanceRaw = cols.balance ? row[cols.balance]?.trim() : undefined;
	const balanceMinor = balanceRaw && balanceRaw.length > 0 ? parseAmountMinor(balanceRaw) : null;

	return {
		chartAccountId,
		postedAt,
		amountMinor,
		currency,
		rawDescription,
		counterparty,
		providerCategory,
		providerTxnId,
		balanceMinor,
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
