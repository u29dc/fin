/**
 * import - Run import pipeline.
 */

import type { ImportResult } from '@fin/core';
import { importInbox } from '@fin/core';
import { initConfig } from '@fin/core/config';
import { defineCommand } from 'citty';

import { resolveDbPath } from '../db';
import { json, log } from '../logger';

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

function renderTableOutput(result: ImportResult): void {
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

	if (result.archivedFiles.length > 0) {
		log(`\nArchived ${result.archivedFiles.length} file${result.archivedFiles.length !== 1 ? 's' : ''}`);
		for (const file of result.archivedFiles.slice(0, 5)) {
			log(`  ${file}`);
		}
		if (result.archivedFiles.length > 5) {
			log(`  ... and ${result.archivedFiles.length - 5} more`);
		}
	}
}

export const importCmd = defineCommand({
	meta: { name: 'import', description: 'Import transactions from inbox' },
	args: {
		inbox: { type: 'string', description: 'Custom inbox directory' },
		format: { type: 'string', description: 'Output format: table, json', default: 'table' },
		db: { type: 'string', description: 'Database path' },
	},
	async run({ args }) {
		initConfig();

		const format = args.format === 'json' ? 'json' : 'table';
		const inboxDir = args.inbox;
		const dbPath = resolveDbPath(args.db ? { options: new Map([['db', args.db]]) } : undefined);

		if (format !== 'json') {
			log('Scanning inbox...\n');
		}

		const options: { inboxDir?: string; dbPath: string; migrate: boolean } = { dbPath, migrate: true };
		if (inboxDir) options.inboxDir = inboxDir;
		const result = await importInbox(options);

		if (format === 'json') {
			json(result);
			return;
		}

		renderTableOutput(result);
	},
});
