import Papa from 'papaparse';
import { getAccountsByProvider, isConfigInitialized } from '../../config/index';
import { parseAmountMinor } from '../../utils/amount';
import { parseWiseDateTime } from '../../utils/datetime';
import type { AssetAccountId, ParsedTransaction, ParseResult } from '../types';

type WiseRow = Record<string, string>;

function isWiseAccount(chartAccountId: string): boolean {
	if (!isConfigInitialized()) {
		// Fallback pattern match when config not loaded
		return chartAccountId.includes(':Wise');
	}
	const wiseAccounts = getAccountsByProvider('wise').map((a) => a.id);
	return wiseAccounts.includes(chartAccountId);
}

export async function parseWiseCsv(filePath: string, chartAccountId: AssetAccountId): Promise<ParseResult> {
	if (!isWiseAccount(chartAccountId)) {
		throw new Error(`Account "${chartAccountId}" is not configured as a Wise account for file: ${filePath}`);
	}

	const text = await Bun.file(filePath).text();
	const result = Papa.parse<WiseRow>(text, {
		header: true,
		skipEmptyLines: true,
	});

	if (result.errors.length > 0) {
		throw new Error(`Wise CSV parse errors: ${result.errors.map((e) => e.message).join('; ')}`);
	}

	const transactions: ParsedTransaction[] = [];

	for (const row of result.data) {
		const providerTxnId = row['TransferWise ID']?.trim() || null;
		const postedAt = parseWiseDateTime(row['Date Time'] ?? '', row['Date']);

		const amountMinor = parseAmountMinor(row['Amount'] ?? '');
		const currency = row['Currency']?.trim() || 'GBP';

		const description = row['Description']?.trim() || '';
		const reference = row['Payment Reference']?.trim() || '';
		const rawDescription = reference.length > 0 ? `${reference} - ${description}`.trim() : description;

		const counterparty = row['Payee Name']?.trim() || row['Payer Name']?.trim() || null;

		const balanceRaw = row['Running Balance']?.trim();
		const balanceMinor = balanceRaw && balanceRaw.length > 0 ? parseAmountMinor(balanceRaw) : null;

		transactions.push({
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
		});
	}

	const hasBalances = true;

	return { chartAccountId, transactions, hasBalances };
}
