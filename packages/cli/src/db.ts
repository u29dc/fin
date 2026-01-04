/**
 * Database connection utilities for CLI.
 *
 * Path resolution priority: --db flag > DB_PATH env > default
 * Read-only commands use create: false to prevent empty DB creation.
 */

import type { Database } from 'bun:sqlite';
import { resolve } from 'node:path';

import { openDatabase } from 'core';

import { getOption, type ParsedArgs } from './args';

const DEFAULT_DB_PATH = 'data/fin.db';

/**
 * Resolve database path from args, env, or default.
 */
export function resolveDbPath(parsed: ParsedArgs): string {
	const fromArg = getOption(parsed, 'db');
	if (fromArg) return resolve(fromArg);

	const fromEnv = process.env['DB_PATH'];
	if (fromEnv) return resolve(fromEnv);

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
export function getReadonlyDb(parsed: ParsedArgs): Database {
	const path = resolveDbPath(parsed);
	return openDatabase({ path, readonly: true, create: false, migrate: true });
}

/**
 * Open database in writable mode.
 * Used by write commands (import, sanitize migrate).
 *
 * - create: true allows initial setup
 * - migrate: true to ensure schema is up to date
 */
export function getWritableDb(parsed: ParsedArgs): Database {
	const path = resolveDbPath(parsed);
	return openDatabase({ path, readonly: false, create: true, migrate: true });
}

/**
 * Open database for sanitize discover (read-only, no create).
 */
export function getDiscoverDb(parsed: ParsedArgs): Database {
	const path = resolveDbPath(parsed);
	return openDatabase({ path, readonly: true, create: false, migrate: true });
}
