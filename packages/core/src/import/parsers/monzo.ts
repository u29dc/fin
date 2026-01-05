import Papa from 'papaparse';
import { getAccountsByProvider, isConfigInitialized } from '../../config/index';
import { parseAmountMinor } from '../../utils/amount';
import { toIsoLocalDateTime } from '../../utils/datetime';
import type { AssetAccountId, ParsedTransaction, ParseResult } from '../types';

type MonzoRow = Record<string, string>;

function isMonzoAccount(chartAccountId: string): boolean {
	if (!isConfigInitialized()) {
		return chartAccountId.includes(':Monzo') || chartAccountId.includes(':Savings');
	}
	const monzoAccounts = getAccountsByProvider('monzo').map((a) => a.id);
	return monzoAccounts.includes(chartAccountId);
}

function parseMonzoRow(row: MonzoRow, chartAccountId: AssetAccountId, filePath: string): ParsedTransaction | null {
	const providerTxnId = row['Transaction ID']?.trim() || null;
	const datePart = row['Date']?.trim();
	const timePart = row['Time']?.trim();

	if (!datePart || !timePart) {
		return null;
	}

	const postedAt = toIsoLocalDateTime(datePart, timePart);
	const amountRaw = row['Amount']?.trim() || row['Money In']?.trim() || row['Money Out']?.trim() || '';
	const amountMinor = parseAmountMinor(amountRaw);
	const currency = row['Currency']?.trim() || 'GBP';

	const name = row['Name']?.trim();
	const description = row['Description']?.trim();
	const rawDescription = (description && description.length > 0 ? description : name) || '';
	const counterparty = name && name.length > 0 ? name : null;
	const providerCategory = row['Category']?.trim() || null;

	const balanceRaw = row['Balance']?.trim();
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

	const text = await Bun.file(filePath).text();
	const result = Papa.parse<MonzoRow>(text, {
		header: true,
		skipEmptyLines: true,
	});

	if (result.errors.length > 0) {
		throw new Error(`Monzo CSV parse errors: ${result.errors.map((e) => e.message).join('; ')}`);
	}

	const transactions: ParsedTransaction[] = [];

	for (const row of result.data) {
		const txn = parseMonzoRow(row, chartAccountId, filePath);
		if (txn) {
			transactions.push(txn);
		}
	}

	const hasBalances = transactions.some((txn) => txn.balanceMinor !== null);

	return { chartAccountId, transactions, hasBalances };
}
