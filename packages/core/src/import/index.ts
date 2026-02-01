import type { Database } from 'bun:sqlite';
import { resolve } from 'node:path';

import { openDatabase } from '../db';
import { migrateToLatest } from '../db/migrate';
import { loadRules, resetRulesCache } from '../sanitize/rules-loader';

import { ArchiveManager } from './archive-manager';
import { createJournalEntriesFromTransactions } from './journal';
import { parseMonzoCsv, parseVanguardCsv, parseVanguardPdf, parseWiseCsv } from './parsers';
import { scanInbox } from './scanner';
import type { CanonicalizationResult } from './transactions';
import { canonicalize } from './transactions';
import type { ArchiveFile, AssetAccountId, DetectedFile, ParsedTransaction } from './types';

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
	totalTransactions: number;
	uniqueTransactions: number;
	duplicateTransactions: number;
	journalEntriesAttempted: number;
	journalEntriesCreated: number;
	transferPairsCreated: number;
	entryErrors: string[];
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

type FileProcessResult = {
	processedFiles: ArchiveFile[];
	skippedFiles: { path: string; reason: string }[];
	parsedTransactions: ParsedTransaction[];
	accountsTouched: AssetAccountId[];
};

function loadImportedSourceFiles(db: Database): Set<string> {
	const rows = db.query<{ source_file: string }, []>('SELECT DISTINCT source_file FROM journal_entries WHERE source_file IS NOT NULL').all();
	return new Set(rows.map((row) => row.source_file));
}

async function processDetectedFiles(detected: DetectedFile[], importedSources: Set<string>): Promise<FileProcessResult> {
	const processedFiles: ArchiveFile[] = [];
	const skippedFiles: { path: string; reason: string }[] = [];
	const parsedTransactions: ParsedTransaction[] = [];
	const accountsTouchedSet = new Set<AssetAccountId>();

	for (const file of detected) {
		if (!file.chartAccountId) {
			skippedFiles.push({ path: file.path, reason: 'Account folder not recognized for this file.' });
			continue;
		}
		if (importedSources.has(file.path)) {
			skippedFiles.push({ path: file.path, reason: 'File already imported.' });
			continue;
		}

		try {
			const parsed = await parseDetectedFile(file);
			processedFiles.push({ path: file.path, provider: file.provider, chartAccountId: parsed.chartAccountId });
			parsedTransactions.push(...parsed.transactions);
			accountsTouchedSet.add(parsed.chartAccountId);
		} catch (error) {
			skippedFiles.push({ path: file.path, reason: error instanceof Error ? error.message : 'Unknown parse error' });
		}
	}

	return { processedFiles, skippedFiles, parsedTransactions, accountsTouched: Array.from(accountsTouchedSet) };
}

async function commitWithArchive(
	db: Database,
	canonResult: CanonicalizationResult,
	processedFiles: ArchiveFile[],
	archiveDir: string,
): Promise<{
	journalResult: ReturnType<typeof createJournalEntriesFromTransactions>;
	archivedFiles: string[];
}> {
	const archiveManager = new ArchiveManager();

	try {
		await archiveManager.prepareArchive(processedFiles, archiveDir);
		const journalResult = createJournalEntriesFromTransactions(db, canonResult.transactions);
		if (journalResult.errors.length > 0) {
			return { journalResult, archivedFiles: [] };
		}
		const archivedFiles = await archiveManager.commitArchive();
		return { journalResult, archivedFiles };
	} catch (error) {
		if (archiveManager.hasArchivedFiles()) {
			await archiveManager.rollbackArchive();
		}
		throw error;
	}
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
	const importedSources = loadImportedSourceFiles(db);
	const { processedFiles, skippedFiles, parsedTransactions, accountsTouched } = await processDetectedFiles(detected, importedSources);

	resetRulesCache();
	const rulesConfig = await loadRules();
	const canonResult = canonicalize(parsedTransactions, rulesConfig);

	const { journalResult, archivedFiles } = await commitWithArchive(db, canonResult, processedFiles, archiveDir);

	return {
		processedFiles: processedFiles.map((file) => file.path),
		archivedFiles,
		skippedFiles,
		totalTransactions: journalResult.totalTransactions,
		uniqueTransactions: journalResult.uniqueTransactions,
		duplicateTransactions: journalResult.duplicateTransactions,
		journalEntriesAttempted: journalResult.entriesAttempted,
		journalEntriesCreated: journalResult.journalEntriesCreated,
		transferPairsCreated: journalResult.transferPairsCreated,
		entryErrors: journalResult.errors,
		accountsTouched,
		unmappedDescriptions: canonResult.unmappedDescriptions,
	};
}
