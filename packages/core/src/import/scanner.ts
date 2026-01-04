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

export async function scanInbox(inboxDir: string): Promise<DetectedFile[]> {
	const entries = await readdir(inboxDir, { withFileTypes: true });
	const detected: DetectedFile[] = [];

	for (const entry of entries) {
		if (!entry.isDirectory()) {
			continue;
		}

		const chartAccountId = chartAccountIdFromFolder(entry.name);
		if (!chartAccountId) {
			continue;
		}

		const accountDir = join(inboxDir, entry.name);
		const accountEntries = await readdir(accountDir, { withFileTypes: true });
		const expectedProvider = getProviderByChartAccount(chartAccountId);

		for (const accountEntry of accountEntries) {
			if (!accountEntry.isFile()) {
				continue;
			}

			const filePath = join(accountDir, accountEntry.name);
			const ext = extname(accountEntry.name).toLowerCase();

			if (ext === '.pdf') {
				if (expectedProvider !== 'vanguard') {
					continue;
				}

				detected.push({
					path: filePath,
					provider: 'vanguard',
					chartAccountId,
				});
				continue;
			}

			if (ext !== '.csv') {
				continue;
			}

			const header = await readFirstLine(filePath);
			const provider = detectProviderFromHeader(header) ?? expectedProvider;
			if (!provider || provider !== expectedProvider) {
				continue;
			}

			detected.push({
				path: filePath,
				provider,
				chartAccountId,
			});
		}
	}

	return detected;
}
