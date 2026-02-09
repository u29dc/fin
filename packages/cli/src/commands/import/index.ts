/**
 * `import` -- Run the import pipeline.
 *
 * Scans inbox, parses CSVs, deduplicates, creates journal entries, and archives.
 * This is a mutating (non-idempotent) command.
 */

import type { ImportResult } from '@fin/core';
import { importInbox } from '@fin/core';
import { resolveDbPath } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { log } from '../../logger';
import { defineToolCommand } from '../../tool';

// ---------------------------------------------------------------------------
// Text mode rendering
// ---------------------------------------------------------------------------

function renderProcessedFiles(files: string[]): void {
	if (files.length === 0) return;
	log('Processed:');
	for (const file of files) {
		log(`  ${file}`);
	}
	log('');
}

function renderSkippedFiles(files: Array<{ path: string; reason: string }>): void {
	if (files.length === 0) return;
	log('Skipped:');
	for (const { path, reason } of files) {
		log(`  ${path}: ${reason}`);
	}
	log('');
}

function renderUnmappedDescriptions(descriptions: string[]): void {
	if (descriptions.length === 0) return;
	log(`\nUnmapped descriptions: ${descriptions.length}`);
	for (const desc of descriptions.slice(0, 10)) {
		log(`  "${desc}"`);
	}
	if (descriptions.length > 10) {
		log(`  ... and ${descriptions.length - 10} more`);
	}
}

function renderArchivedFiles(files: string[]): void {
	if (files.length === 0) return;
	log(`\nArchived ${files.length} file${files.length !== 1 ? 's' : ''}`);
	for (const file of files.slice(0, 5)) {
		log(`  ${file}`);
	}
	if (files.length > 5) {
		log(`  ... and ${files.length - 5} more`);
	}
}

function renderTextOutput(result: ImportResult): void {
	renderProcessedFiles(result.processedFiles);
	renderSkippedFiles(result.skippedFiles);

	log('Results:');
	log(`  Transactions parsed: ${result.totalTransactions}`);
	if (result.duplicateTransactions > 0) {
		log(`  Duplicates skipped: ${result.duplicateTransactions}`);
	}
	log(`  Journal entries attempted: ${result.journalEntriesAttempted}`);
	log(`  Journal entries created: ${result.journalEntriesCreated}`);
	if (result.transferPairsCreated > 0) {
		log(`  Transfer pairs created: ${result.transferPairsCreated}`);
	}
	if (result.entryErrors.length > 0) {
		log(`  Entry errors: ${result.entryErrors.length}`);
		for (const err of result.entryErrors.slice(0, 10)) {
			log(`    ${err}`);
		}
		if (result.entryErrors.length > 10) {
			log(`    ... and ${result.entryErrors.length - 10} more`);
		}
	}

	if (result.accountsTouched.length > 0) {
		log(`  Accounts touched: ${result.accountsTouched.join(', ')}`);
	}

	renderUnmappedDescriptions(result.unmappedDescriptions);
	renderArchivedFiles(result.archivedFiles);
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

export const importCommand = defineToolCommand(
	{
		name: 'import',
		command: 'fin import',
		category: 'import',
		outputFields: [
			'processedFiles',
			'skippedFiles',
			'totalTransactions',
			'duplicateTransactions',
			'journalEntriesCreated',
			'transferPairsCreated',
			'entryErrors',
			'accountsTouched',
			'unmappedDescriptions',
			'archivedFiles',
		],
		idempotent: false,
		rateLimit: null,
		example: 'fin import --json',
	},
	{
		meta: {
			name: 'import',
			description: 'Import transactions from inbox',
		},
		args: {
			inbox: { type: 'string' as const, description: 'Custom inbox directory' },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
		},
		async run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				const dbPath = resolveDbPath(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const inboxDir = args.inbox;

				if (!jsonMode) {
					log('Scanning inbox...\n');
				}

				const options: { inboxDir?: string; dbPath: string; migrate: boolean } = { dbPath, migrate: true };
				if (inboxDir) options.inboxDir = inboxDir;
				const result = await importInbox(options);

				if (jsonMode) {
					ok(
						'import',
						{
							processedFiles: result.processedFiles,
							skippedFiles: result.skippedFiles,
							totalTransactions: result.totalTransactions,
							duplicateTransactions: result.duplicateTransactions,
							journalEntriesCreated: result.journalEntriesCreated,
							transferPairsCreated: result.transferPairsCreated,
							entryErrors: result.entryErrors,
							accountsTouched: result.accountsTouched,
							unmappedDescriptions: result.unmappedDescriptions,
							archivedFiles: result.archivedFiles,
						},
						start,
						{ count: result.journalEntriesCreated },
					);
				}

				renderTextOutput(result);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('import', 'IMPORT_ERROR', `Import failed: ${message}`, 'Check imports/inbox/ structure and data/fin.config.toml', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
