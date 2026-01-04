/**
 * import - Run import pipeline.
 */

import type { ImportResult } from 'core';
import { importInbox } from 'core';
import { initConfig } from 'core/config';

import { getOption, parseArgs } from '../args';
import { resolveDbPath } from '../db';

function renderProcessedFiles(files: string[]): void {
	if (files.length === 0) return;
	console.log('Processed:');
	for (const file of files) {
		console.log(`  ${file}`);
	}
	console.log('');
}

function renderSkippedFiles(files: Array<{ path: string; reason: string }>): void {
	if (files.length === 0) return;
	console.log('Skipped:');
	for (const { path, reason } of files) {
		console.log(`  ${path}: ${reason}`);
	}
	console.log('');
}

function renderUnmappedDescriptions(descriptions: string[]): void {
	if (descriptions.length === 0) return;
	console.log(`\nUnmapped descriptions: ${descriptions.length}`);
	for (const desc of descriptions.slice(0, 10)) {
		console.log(`  "${desc}"`);
	}
	if (descriptions.length > 10) {
		console.log(`  ... and ${descriptions.length - 10} more`);
	}
}

function renderTableOutput(result: ImportResult): void {
	renderProcessedFiles(result.processedFiles);
	renderSkippedFiles(result.skippedFiles);

	console.log('Results:');
	console.log(`  Journal entries created: ${result.journalEntriesCreated}`);

	if (result.accountsTouched.length > 0) {
		console.log(`  Accounts touched: ${result.accountsTouched.join(', ')}`);
	}

	renderUnmappedDescriptions(result.unmappedDescriptions);

	if (result.archivedFiles.length > 0) {
		console.log(`\nArchived ${result.archivedFiles.length} file${result.archivedFiles.length !== 1 ? 's' : ''}`);
	}
}

export async function runImport(args: string[]): Promise<void> {
	initConfig();

	const parsed = parseArgs(args);
	const formatRaw = getOption(parsed, 'format');
	if (formatRaw && formatRaw !== 'json' && formatRaw !== 'table') {
		console.error('Invalid format. Use: table, json');
		process.exit(1);
	}

	const format = formatRaw === 'json' ? 'json' : 'table';
	const inboxDir = getOption(parsed, 'inbox');
	const dbPath = resolveDbPath(parsed);

	if (format !== 'json') {
		console.log('Scanning inbox...\n');
	}

	const options: { inboxDir?: string; dbPath: string; migrate: boolean } = { dbPath, migrate: true };
	if (inboxDir) options.inboxDir = inboxDir;
	const result = await importInbox(options);

	if (format === 'json') {
		console.log(JSON.stringify(result, null, 2));
		return;
	}

	renderTableOutput(result);
}
