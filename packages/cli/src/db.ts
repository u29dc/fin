/**
 * Database connection utilities for CLI.
 *
 * Path resolution priority: --db flag > DB_PATH env > config dir > cwd
 * Read-only commands use create: false to prevent empty DB creation.
 */

import type { Database } from 'bun:sqlite';
import { resolve } from 'node:path';

import { openDatabase } from 'core';
import { getConfigDir } from 'core/config';

const DEFAULT_DB_PATH = 'data/fin.db';

type DbArgs = { options?: Map<string, string> } | undefined;

function getOption(args: DbArgs, name: string): string | undefined {
	return args?.options?.get(name);
}

/**
 * Resolve database path from args, env, or default.
 */
export function resolveDbPath(args?: DbArgs): string {
	const fromArg = getOption(args, 'db');
	if (fromArg) return resolve(fromArg);

	const fromEnv = process.env['DB_PATH'];
	if (fromEnv) return resolve(fromEnv);

	// Use config directory (same as fin.config.toml location)
	const configDir = getConfigDir();
	if (configDir) {
		return resolve(configDir, 'fin.db');
	}

	return resolve(process.cwd(), DEFAULT_DB_PATH);
}

/**
 * Open database in read-only mode.
 * Used by query commands (accounts, transactions, cashflow, etc.)
 *
 * - create: false prevents accidental empty DB creation
 * - readonly: true for safety
 * - migrate: true to ensure schema is up to date
 */
export function getReadonlyDb(args?: DbArgs): Database {
	const path = resolveDbPath(args);
	return openDatabase({ path, readonly: true, create: false, migrate: true });
}

/**
 * Open database in writable mode.
 * Used by write commands (import, sanitize migrate).
 *
 * - create: true allows initial setup
 * - migrate: true to ensure schema is up to date
 */
export function getWritableDb(args?: DbArgs): Database {
	const path = resolveDbPath(args);
	return openDatabase({ path, readonly: false, create: true, migrate: true });
}

/**
 * Open database for sanitize discover (read-only, no create).
 */
export function getDiscoverDb(args?: DbArgs): Database {
	const path = resolveDbPath(args);
	return openDatabase({ path, readonly: true, create: false, migrate: true });
}
