import { resolve } from 'node:path';

import { openDatabase } from '../db';
import { migrateToLatest } from '../db/migrate';
import { loadRules, resetRulesCache } from '../sanitize/rules-loader';

import { ArchiveManager } from './archive-manager';
import { createJournalEntriesFromTransactions } from './journal-entry-creator';
import { parseMonzoCsv, parseVanguardCsv, parseVanguardPdf, parseWiseCsv } from './parsers';
import { scanInbox } from './scanner';
import { canonicalize } from './transactions';
import type { AssetAccountId, DetectedFile, ParsedTransaction } from './types';

export type ImportInboxOptions = {
	inboxDir?: string;
	archiveDir?: string;
	dbPath?: string;
	migrate?: boolean;
};

export type ImportResult = {
	processedFiles: string[];
	archivedFiles: string[];
	skippedFiles: { path: string; reason: string }[];
	journalEntriesCreated: number;
	accountsTouched: AssetAccountId[];
	unmappedDescriptions: string[];
};

export type { AssetAccountId } from './types';
export { isAssetAccountId } from './types';

async function parseDetectedFile(file: DetectedFile) {
	if (!file.chartAccountId) {
		throw new Error('Missing chart account id for detected file');
	}

	if (file.provider === 'monzo') {
		return parseMonzoCsv(file.path, file.chartAccountId);
	}
	if (file.provider === 'wise') {
		return parseWiseCsv(file.path, file.chartAccountId);
	}
	if (file.provider === 'vanguard') {
		if (file.path.toLowerCase().endsWith('.csv')) {
			return parseVanguardCsv(file.path, file.chartAccountId);
		}

		if (file.path.toLowerCase().endsWith('.pdf')) {
			return parseVanguardPdf(file.path, file.chartAccountId);
		}

		throw new Error(`Unsupported Vanguard file: ${file.path}`);
	}

	throw new Error(`Unsupported provider: ${file.provider}`);
}

export async function importInbox(options: ImportInboxOptions = {}): Promise<ImportResult> {
	const inboxDir = options.inboxDir ?? resolve(process.cwd(), 'imports/inbox');
	const archiveDir = options.archiveDir ?? resolve(process.cwd(), 'imports/archive');
	const dbPath = options.dbPath ?? resolve(process.cwd(), 'data/fin.db');
	const shouldMigrate = options.migrate ?? true;

	const db = openDatabase({ path: dbPath, create: true, migrate: false });
	if (shouldMigrate) {
		migrateToLatest(db);
	}

	const detected = await scanInbox(inboxDir);
	const processedFiles: string[] = [];
	const skippedFiles: { path: string; reason: string }[] = [];

	const parsedTransactions: ParsedTransaction[] = [];
	const accountsTouchedSet = new Set<AssetAccountId>();

	for (const file of detected) {
		if (!file.chartAccountId) {
			skippedFiles.push({
				path: file.path,
				reason: 'Account folder not recognized for this file.',
			});
			continue;
		}

		try {
			const parsed = await parseDetectedFile(file);
			processedFiles.push(file.path);
			for (const txn of parsed.transactions) {
				parsedTransactions.push(txn);
			}
			accountsTouchedSet.add(parsed.chartAccountId);
		} catch (error) {
			skippedFiles.push({
				path: file.path,
				reason: error instanceof Error ? error.message : 'Unknown parse error',
			});
		}
	}

	resetRulesCache();
	const rulesConfig = await loadRules();
	const canonResult = canonicalize(parsedTransactions, rulesConfig);
	const accountsTouched = Array.from(accountsTouchedSet.values());

	// Two-phase commit: prepare archive first, then DB operations, then commit archive
	const archiveManager = new ArchiveManager();
	let journalEntriesCreated = 0;
	let archivedFiles: string[] = [];

	try {
		// Phase 1: Prepare archive (creates target dir, but doesn't move files yet)
		await archiveManager.prepareArchive(processedFiles, archiveDir);

		// Phase 2: DB operations (can fail safely, no files moved yet)
		const journalResult = createJournalEntriesFromTransactions(db, canonResult.transactions);
		journalEntriesCreated = journalResult.journalEntriesCreated;

		// Phase 3: Commit archive (move files only after DB succeeds)
		archivedFiles = await archiveManager.commitArchive();
	} catch (error) {
		// Rollback any archived files if something failed after archiving started
		if (archiveManager.hasArchivedFiles()) {
			await archiveManager.rollbackArchive();
		}
		throw error;
	}

	return {
		processedFiles,
		archivedFiles,
		skippedFiles,
		journalEntriesCreated,
		accountsTouched,
		unmappedDescriptions: canonResult.unmappedDescriptions,
	};
}
