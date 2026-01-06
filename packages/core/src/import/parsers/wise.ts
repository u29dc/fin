import Papa from 'papaparse';
import { getAccountsByProvider, isConfigInitialized } from '../../config/index';
import { parseAmountMinor } from '../../utils/amount';
import { parseWiseDateTime } from '../../utils/datetime';
import type { AssetAccountId, ParsedTransaction, ParseResult } from '../types';
import { type ColumnMapping, getColumnMapping, validateCsvHeaders } from './validation';

type WiseRow = Record<string, string>;

function isWiseAccount(chartAccountId: string): boolean {
	if (!isConfigInitialized()) {
		return chartAccountId.includes(':Wise');
	}
	const wiseAccounts = getAccountsByProvider('wise').map((a) => a.id);
	return wiseAccounts.includes(chartAccountId);
}

function parseWiseRow(row: WiseRow, cols: ColumnMapping, chartAccountId: AssetAccountId, filePath: string): ParsedTransaction {
	// Use config-defined column names
	const providerTxnId = (cols.transactionId ? row[cols.transactionId]?.trim() : null) || null;

	// Wise has two date formats: "Date Time" or separate "Date"
	const dateTime = row['Date Time'] ?? '';
	const dateOnly = row[cols.date] ?? '';
	const postedAt = parseWiseDateTime(dateTime, dateOnly);

	const amountMinor = parseAmountMinor(row[cols.amount] ?? '');
	const currency = row['Currency']?.trim() || 'GBP';

	const description = row[cols.description]?.trim() || '';
	const reference = row['Payment Reference']?.trim() || '';
	const rawDescription = reference.length > 0 ? `${reference} - ${description}`.trim() : description;

	const counterparty = row['Payee Name']?.trim() || row['Payer Name']?.trim() || null;

	const balanceRaw = cols.balance ? row[cols.balance]?.trim() : undefined;
	const balanceMinor = balanceRaw && balanceRaw.length > 0 ? parseAmountMinor(balanceRaw) : null;

	return {
		chartAccountId,
		postedAt,
		amountMinor,
		currency,
		rawDescription,
		counterparty,
		providerCategory: row['Transaction Type']?.trim() || null,
		providerTxnId,
		balanceMinor,
		sourceFile: filePath,
	};
}

export async function parseWiseCsv(filePath: string, chartAccountId: AssetAccountId): Promise<ParseResult> {
	if (!isWiseAccount(chartAccountId)) {
		throw new Error(`Account "${chartAccountId}" is not configured as a Wise account for file: ${filePath}`);
	}

	// Get column mapping from config
	const cols = getColumnMapping('wise');

	const text = await Bun.file(filePath).text();
	const result = Papa.parse<WiseRow>(text, {
		header: true,
		skipEmptyLines: true,
	});

	if (result.errors.length > 0) {
		throw new Error(`Wise CSV parse errors: ${result.errors.map((e) => e.message).join('; ')}`);
	}

	// Validate headers against config-defined required columns
	const headers = result.meta.fields ?? [];
	validateCsvHeaders(headers, 'wise');

	const transactions = result.data.map((row) => parseWiseRow(row, cols, chartAccountId, filePath));

	// Fix: actually check if balances are present instead of hardcoded true
	const hasBalances = transactions.some((txn) => txn.balanceMinor !== null);

	return { chartAccountId, transactions, hasBalances };
}
