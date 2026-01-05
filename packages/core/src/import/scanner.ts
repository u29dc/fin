import { readdir } from 'node:fs/promises';
import { extname, join } from 'node:path';
import { getInboxFolderToChartId, getProviderForAccount, isConfigInitialized } from '../config/index';
import type { AssetAccountId, DetectedFile, DetectedProvider } from './types';

async function readFirstLine(filePath: string): Promise<string> {
	const text = await Bun.file(filePath)
		.slice(0, 8 * 1024)
		.text();
	const [firstLine] = text.split(/\r?\n/);
	return firstLine ?? '';
}

function getProviderByChartAccount(chartAccountId: string): DetectedProvider | null {
	if (!isConfigInitialized()) {
		return null;
	}
	const provider = getProviderForAccount(chartAccountId);
	return provider as DetectedProvider | null;
}

function chartAccountIdFromFolder(folderName: string): AssetAccountId | null {
	if (!isConfigInitialized()) {
		return null;
	}
	const mappings = getInboxFolderToChartId();
	return (mappings[folderName] as AssetAccountId) ?? null;
}

function detectProviderFromHeader(header: string): DetectedProvider | null {
	if (header.includes('TransferWise ID')) {
		return 'wise';
	}
	if (header.includes('Transaction ID') && header.includes('Money Out')) {
		return 'monzo';
	}
	return null;
}

function tryDetectPdf(ext: string, expectedProvider: DetectedProvider | null, filePath: string, chartAccountId: AssetAccountId): DetectedFile | null {
	if (ext !== '.pdf') return null;
	if (expectedProvider !== 'vanguard') return null;

	return { path: filePath, provider: 'vanguard', chartAccountId };
}

async function tryDetectCsv(ext: string, expectedProvider: DetectedProvider | null, filePath: string, chartAccountId: AssetAccountId): Promise<DetectedFile | null> {
	if (ext !== '.csv') return null;

	const header = await readFirstLine(filePath);
	const provider = detectProviderFromHeader(header) ?? expectedProvider;

	if (!provider || provider !== expectedProvider) return null;

	return { path: filePath, provider, chartAccountId };
}

async function scanAccountDirectory(accountDir: string, chartAccountId: AssetAccountId, expectedProvider: DetectedProvider | null): Promise<DetectedFile[]> {
	const accountEntries = await readdir(accountDir, { withFileTypes: true });
	const detected: DetectedFile[] = [];

	for (const accountEntry of accountEntries) {
		if (!accountEntry.isFile()) continue;

		const filePath = join(accountDir, accountEntry.name);
		const ext = extname(accountEntry.name).toLowerCase();

		const pdfResult = tryDetectPdf(ext, expectedProvider, filePath, chartAccountId);
		if (pdfResult) {
			detected.push(pdfResult);
			continue;
		}

		const csvResult = await tryDetectCsv(ext, expectedProvider, filePath, chartAccountId);
		if (csvResult) {
			detected.push(csvResult);
		}
	}

	return detected;
}

export async function scanInbox(inboxDir: string): Promise<DetectedFile[]> {
	const entries = await readdir(inboxDir, { withFileTypes: true });
	const detected: DetectedFile[] = [];

	for (const entry of entries) {
		if (!entry.isDirectory()) continue;

		const chartAccountId = chartAccountIdFromFolder(entry.name);
		if (!chartAccountId) continue;

		const accountDir = join(inboxDir, entry.name);
		const expectedProvider = getProviderByChartAccount(chartAccountId);
		const accountFiles = await scanAccountDirectory(accountDir, chartAccountId, expectedProvider);
		detected.push(...accountFiles);
	}

	return detected;
}
