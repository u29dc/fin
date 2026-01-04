/**
 * Backup Script
 * Backs up database, config, and rules to timestamped folder.
 *
 * Usage: bun run backup
 */

/* biome-ignore-all lint/suspicious/noConsole: CLI script requires console output */

import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { resolve } from 'node:path';

const DATA_DIR = resolve(process.cwd(), 'data');
const BACKUPS_DIR = resolve(DATA_DIR, 'backups');

const FILES_TO_BACKUP = ['fin.db', 'fin.config.toml', 'fin.rules.ts'];

function getTimestamp(): string {
	return new Date().toISOString().replace(/[-:]/g, '').replace('T', '-').split('.')[0] ?? 'unknown';
}

function main(): void {
	const timestamp = getTimestamp();
	const backupDir = resolve(BACKUPS_DIR, timestamp);

	console.log(`Backing up to ${backupDir}`);

	// Create backup directory
	mkdirSync(backupDir, { recursive: true });

	let backedUp = 0;
	for (const file of FILES_TO_BACKUP) {
		const src = resolve(DATA_DIR, file);
		const dest = resolve(backupDir, file);

		if (existsSync(src)) {
			copyFileSync(src, dest);
			console.log(`  ${file}`);
			backedUp++;
		} else {
			console.log(`  ${file} (skipped - not found)`);
		}
	}

	console.log(`\nBacked up ${backedUp} files to data/backups/${timestamp}/`);
}

main();
