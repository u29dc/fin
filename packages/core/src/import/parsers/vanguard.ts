import Papa from 'papaparse';
import { getAccountsByProvider, isConfigInitialized } from '../../config/index';
import { parseAmountMinor } from '../../utils/amount';
import type { AssetAccountId, ParsedTransaction, ParseResult } from '../types';
import { getColumnMapping, validateCsvHeaders } from './validation';

function isVanguardAccount(chartAccountId: string): boolean {
	if (!isConfigInitialized()) {
		throw new Error('Config must be initialized before parsing Vanguard files');
	}
	const vanguardAccounts = getAccountsByProvider('vanguard').map((a) => a.id);
	return vanguardAccounts.includes(chartAccountId);
}

type VanguardRow = Record<string, string>;

function pad2(value: number): string {
	return value.toString().padStart(2, '0');
}

function toIsoDateTime(date: string): string {
	const trimmed = date.trim();
	if (!/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) {
		throw new Error(`Invalid Vanguard date: ${date}`);
	}

	return `${trimmed}T00:00:00`;
}

function isExternalCashMovement(detailsRaw: string): boolean {
	const details = detailsRaw.trim().toLowerCase();
	if (details.length === 0) {
		return false;
	}

	if (details.startsWith('bought ') || details.startsWith('sold ')) {
		return false;
	}

	// This represents growth, not cash in/out.
	if (details.includes('interest')) {
		return false;
	}

	// Include deposits (incl reversals like "REV: Regular Deposit ...").
	if (details.includes('deposit')) {
		return true;
	}

	// Include explicit transfers out of Vanguard.
	if (details.startsWith('funds transferred')) {
		return true;
	}

	if (details.includes('withdraw')) {
		return true;
	}

	return false;
}

function parseEnglishDateToIso(dateRaw: string): string {
	const match = dateRaw.trim().match(/^(\d{1,2})\s+([A-Za-z]+)\s+(\d{4})$/);
	if (!match) {
		throw new Error(`Invalid Vanguard valuation date: ${dateRaw}`);
	}

	const day = Number(match[1]);
	const monthName = match[2]?.toLowerCase();
	const year = Number(match[3]);
	if (!Number.isFinite(day) || !Number.isFinite(year) || !monthName) {
		throw new Error(`Invalid Vanguard valuation date: ${dateRaw}`);
	}

	const monthMap: Record<string, number> = {
		january: 1,
		february: 2,
		march: 3,
		april: 4,
		may: 5,
		june: 6,
		july: 7,
		august: 8,
		september: 9,
		october: 10,
		november: 11,
		december: 12,
	};

	const month = monthMap[monthName];
	if (!month) {
		throw new Error(`Invalid Vanguard valuation month: ${monthName}`);
	}

	return `${year}-${pad2(month)}-${pad2(day)}`;
}

function extractVanguardPortfolioValueSnapshot(text: string): { date: string; valueMinor: number } {
	const dateMatch = text.match(/Portfolio Value by Product Wrapper as at ([0-9]{1,2} [A-Za-z]+ [0-9]{4})/) ?? text.match(/\n([0-9]{1,2} [A-Za-z]+ [0-9]{4})\n/);

	const valuationDateRaw = dateMatch?.[1];
	if (!valuationDateRaw) {
		throw new Error('Could not find Vanguard valuation date in PDF text.');
	}

	const date = parseEnglishDateToIso(valuationDateRaw);

	const anchor = 'Total Portfolio Value';
	const anchorIndex = text.indexOf(anchor);
	if (anchorIndex === -1) {
		throw new Error('Could not find "Total Portfolio Value" section in Vanguard PDF text.');
	}

	const windowText = text.slice(anchorIndex, anchorIndex + 1_000);
	const valueMatch = windowText.match(/£\s*([0-9,]+\.[0-9]{2})/);
	const valueRaw = valueMatch?.[1];
	if (!valueRaw) {
		throw new Error('Could not find a GBP value near "Total Portfolio Value" in PDF text.');
	}

	return { date, valueMinor: parseAmountMinor(valueRaw) };
}

export async function parseVanguardCsv(filePath: string, chartAccountId: AssetAccountId): Promise<ParseResult> {
	if (!isVanguardAccount(chartAccountId)) {
		throw new Error(`Account "${chartAccountId}" is not configured as a Vanguard account for file: ${filePath}`);
	}

	// Get column mapping from config
	const cols = getColumnMapping('vanguard');

	const text = await Bun.file(filePath).text();
	const result = Papa.parse<VanguardRow>(text, {
		header: true,
		skipEmptyLines: true,
	});

	if (result.errors.length > 0) {
		throw new Error(`Vanguard CSV parse errors: ${result.errors.map((e) => e.message).join('; ')}`);
	}

	// Validate headers against config-defined required columns
	const headers = result.meta.fields ?? [];
	validateCsvHeaders(headers, 'vanguard');

	const transactions: ParsedTransaction[] = [];

	for (const row of result.data) {
		// Use config-defined column names
		const datePart = row[cols.date]?.trim();
		const details = row[cols.description]?.trim() || '';
		const amountRaw = row[cols.amount]?.trim();

		if (!datePart || !amountRaw) {
			continue;
		}

		if (!isExternalCashMovement(details)) {
			continue;
		}

		// Generate stable ID for deduplication: date-description-amount
		const providerTxnId = `vanguard-csv-${datePart}-${details.toLowerCase().replace(/\s+/g, '-').slice(0, 50)}-${amountRaw}`;

		transactions.push({
			chartAccountId,
			postedAt: toIsoDateTime(datePart),
			amountMinor: parseAmountMinor(amountRaw),
			currency: 'GBP',
			rawDescription: details,
			counterparty: null,
			providerCategory: null,
			providerTxnId,
			balanceMinor: null,
			sourceFile: filePath,
		});
	}

	return { chartAccountId, transactions, hasBalances: false };
}

export async function parseVanguardPdf(filePath: string, chartAccountId: AssetAccountId): Promise<ParseResult> {
	if (!isVanguardAccount(chartAccountId)) {
		throw new Error(`Account "${chartAccountId}" is not configured as a Vanguard account for file: ${filePath}`);
	}

	const result = Bun.spawnSync({
		cmd: ['pdftotext', filePath, '-'],
		stdout: 'pipe',
		stderr: 'pipe',
	});

	if (result.exitCode !== 0) {
		const stderr = new TextDecoder().decode(result.stderr);
		throw new Error(`Failed to extract text from Vanguard PDF via pdftotext (exit ${result.exitCode}): ${stderr.trim()}`);
	}

	const text = new TextDecoder().decode(result.stdout);
	const snapshot = extractVanguardPortfolioValueSnapshot(text);

	const transactions: ParsedTransaction[] = [
		{
			chartAccountId,
			postedAt: `${snapshot.date}T00:00:00`,
			amountMinor: 0,
			currency: 'GBP',
			rawDescription: `Vanguard portfolio valuation (${snapshot.date})`,
			counterparty: null,
			providerCategory: 'portfolio_valuation',
			providerTxnId: `vanguard-valuation-${snapshot.date}`,
			balanceMinor: snapshot.valueMinor,
			sourceFile: filePath,
		},
	];

	return { chartAccountId, transactions, hasBalances: true };
}
