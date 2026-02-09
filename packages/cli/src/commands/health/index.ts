/**
 * `health` -- Prerequisites and system health check.
 *
 * Checks config, database, rules, inbox directory.
 * Uses plain defineCommand() -- infrastructure, not a tool.
 */

import { Database } from 'bun:sqlite';
import { existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { SCHEMA_VERSION } from '@fin/core';
import { loadConfig } from '@fin/core/config';
import type { ArgsDef } from 'citty';
import { defineCommand } from 'citty';
import { emitRaw, isJsonMode } from '../../envelope';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type CheckStatus = 'ok' | 'missing' | 'invalid';
type Severity = 'blocking' | 'degraded' | 'info';

interface HealthCheck {
	id: string;
	label: string;
	status: CheckStatus;
	severity: Severity;
	detail: string | null;
	fix: string[] | null;
}

// ---------------------------------------------------------------------------
// Path resolution (standalone, no initConfig dependency)
// ---------------------------------------------------------------------------

function findMonorepoRoot(startDir: string): string | null {
	let dir = startDir;
	while (dir !== dirname(dir)) {
		if (existsSync(resolve(dir, 'fin.config.template.toml'))) {
			return dir;
		}
		dir = dirname(dir);
	}
	return null;
}

function resolveConfigPath(): string {
	const envPath = process.env['FIN_CONFIG_PATH'];
	if (envPath) return resolve(envPath);

	const homeDir = process.env['FIN_HOME'];
	if (homeDir) return resolve(homeDir, 'data', 'fin.config.toml');

	const root = findMonorepoRoot(process.cwd());
	if (root) return resolve(root, 'data', 'fin.config.toml');

	return resolve(process.cwd(), 'data', 'fin.config.toml');
}

function resolveDbPath(configDir: string): string {
	const fromEnv = process.env['DB_PATH'];
	if (fromEnv) return resolve(fromEnv);
	return resolve(configDir, 'fin.db');
}

function resolveRulesPath(configDir: string): string {
	// Default rules path relative to config dir (data/ -> data/fin.rules.ts)
	return resolve(configDir, 'fin.rules.ts');
}

function resolveInboxPath(): string {
	const root = findMonorepoRoot(process.cwd());
	if (root) return resolve(root, 'imports', 'inbox');
	return resolve(process.cwd(), 'imports', 'inbox');
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

function checkConfigExists(configPath: string): HealthCheck {
	if (!existsSync(configPath)) {
		return {
			id: 'config',
			label: 'Configuration',
			status: 'missing',
			severity: 'blocking',
			detail: configPath,
			fix: ['cp fin.config.template.toml data/fin.config.toml'],
		};
	}
	return {
		id: 'config',
		label: 'Configuration',
		status: 'ok',
		severity: 'info',
		detail: configPath,
		fix: null,
	};
}

function checkConfigValidates(configPath: string): HealthCheck {
	if (!existsSync(configPath)) {
		return {
			id: 'config_valid',
			label: 'Configuration validates',
			status: 'missing',
			severity: 'blocking',
			detail: 'Config file missing, cannot validate',
			fix: ['cp fin.config.template.toml data/fin.config.toml'],
		};
	}

	try {
		loadConfig(configPath);
		return {
			id: 'config_valid',
			label: 'Configuration validates',
			status: 'ok',
			severity: 'info',
			detail: configPath,
			fix: null,
		};
	} catch (e) {
		const msg = e instanceof Error ? e.message : String(e);
		return {
			id: 'config_valid',
			label: 'Configuration validates',
			status: 'invalid',
			severity: 'blocking',
			detail: `${configPath} -- ${msg}`,
			fix: ['cp fin.config.template.toml data/fin.config.toml'],
		};
	}
}

function checkDatabase(dbPath: string): HealthCheck {
	if (!existsSync(dbPath)) {
		return {
			id: 'database',
			label: 'Database',
			status: 'missing',
			severity: 'info',
			detail: `${dbPath} (created on first import)`,
			fix: ['bun run fin import'],
		};
	}
	return {
		id: 'database',
		label: 'Database',
		status: 'ok',
		severity: 'info',
		detail: dbPath,
		fix: null,
	};
}

const REQUIRED_TABLES = ['chart_of_accounts', 'journal_entries', 'postings'] as const;

function checkDbSchema(dbPath: string): HealthCheck | null {
	if (!existsSync(dbPath)) return null;

	try {
		const db = new Database(dbPath, { readonly: true });
		try {
			const versionRow = db.query<{ user_version: number }, []>('PRAGMA user_version').get();
			const currentVersion = versionRow?.user_version ?? 0;

			const tableRows = db
				.query<{ name: string }, []>(`SELECT name FROM sqlite_master WHERE type='table' AND name IN (${REQUIRED_TABLES.map(() => '?').join(',')})`)
				.all(...(REQUIRED_TABLES as unknown as []));
			const foundTables = new Set(tableRows.map((r) => r.name));
			const missingTables = REQUIRED_TABLES.filter((t) => !foundTables.has(t));

			if (currentVersion !== SCHEMA_VERSION || missingTables.length > 0) {
				const details: string[] = [];
				if (currentVersion !== SCHEMA_VERSION) {
					details.push(`version ${currentVersion}, expected ${SCHEMA_VERSION}`);
				}
				if (missingTables.length > 0) {
					details.push(`missing tables: ${missingTables.join(', ')}`);
				}
				return {
					id: 'db_schema',
					label: 'Database schema',
					status: 'invalid',
					severity: 'blocking',
					detail: details.join('; '),
					fix: ['bun run fin import'],
				};
			}

			return {
				id: 'db_schema',
				label: 'Database schema',
				status: 'ok',
				severity: 'info',
				detail: `version ${currentVersion}, ${REQUIRED_TABLES.length} tables`,
				fix: null,
			};
		} finally {
			db.close();
		}
	} catch (e) {
		const msg = e instanceof Error ? e.message : String(e);
		return {
			id: 'db_schema',
			label: 'Database schema',
			status: 'invalid',
			severity: 'blocking',
			detail: `failed to read schema: ${msg}`,
			fix: ['bun run fin import'],
		};
	}
}

function checkRules(rulesPath: string): HealthCheck {
	if (!existsSync(rulesPath)) {
		return {
			id: 'rules',
			label: 'Rules file',
			status: 'missing',
			severity: 'degraded',
			detail: rulesPath,
			fix: ['cp fin.rules.template.ts data/fin.rules.ts'],
		};
	}
	return {
		id: 'rules',
		label: 'Rules file',
		status: 'ok',
		severity: 'info',
		detail: rulesPath,
		fix: null,
	};
}

function checkInbox(inboxPath: string): HealthCheck {
	if (!existsSync(inboxPath)) {
		return {
			id: 'inbox',
			label: 'Inbox directory',
			status: 'missing',
			severity: 'info',
			detail: inboxPath,
			fix: [`mkdir -p ${inboxPath}`],
		};
	}
	return {
		id: 'inbox',
		label: 'Inbox directory',
		status: 'ok',
		severity: 'info',
		detail: inboxPath,
		fix: null,
	};
}

// ---------------------------------------------------------------------------
// Main health check
// ---------------------------------------------------------------------------

function runHealthChecks(): { checks: HealthCheck[]; status: 'ready' | 'degraded' | 'blocked' } {
	const configPath = resolveConfigPath();
	const configDir = dirname(configPath);
	const dbPath = resolveDbPath(configDir);
	const rulesPath = resolveRulesPath(configDir);
	const inboxPath = resolveInboxPath();

	const checks: HealthCheck[] = [];
	checks.push(checkConfigExists(configPath));
	checks.push(checkConfigValidates(configPath));
	checks.push(checkDatabase(dbPath));
	const schemaCheck = checkDbSchema(dbPath);
	if (schemaCheck) checks.push(schemaCheck);
	checks.push(checkRules(rulesPath));
	checks.push(checkInbox(inboxPath));

	const hasBlocking = checks.some((c) => c.severity === 'blocking' && c.status !== 'ok');
	const hasDegraded = checks.some((c) => c.severity === 'degraded' && c.status !== 'ok');
	const status = hasBlocking ? 'blocked' : hasDegraded ? 'degraded' : 'ready';

	return { checks, status };
}

// ---------------------------------------------------------------------------
// Summary + text output
// ---------------------------------------------------------------------------

function computeSummary(checks: HealthCheck[]): { ok: number; blocking: number; degraded: number } {
	return {
		ok: checks.filter((c) => c.status === 'ok').length,
		blocking: checks.filter((c) => c.severity === 'blocking' && c.status !== 'ok').length,
		degraded: checks.filter((c) => c.severity === 'degraded' && c.status !== 'ok').length,
	};
}

function checkIcon(check: HealthCheck): string {
	if (check.status === 'ok') return '+';
	if (check.severity === 'blocking') return 'x';
	return '!';
}

function statusLabel(status: string): string {
	if (status === 'ready') return 'READY';
	if (status === 'degraded') return 'DEGRADED';
	return 'BLOCKED';
}

function printTextOutput(checks: HealthCheck[], status: string, summary: { ok: number; blocking: number; degraded: number }): void {
	process.stderr.write(`Health: ${statusLabel(status)}\n`);
	for (const check of checks) {
		const icon = checkIcon(check);
		const detail = check.detail ? ` (${check.detail})` : '';
		process.stderr.write(`  ${icon} ${check.label}: ${check.status}${detail}\n`);
		if (check.fix && check.status !== 'ok') {
			for (const cmd of check.fix) {
				process.stderr.write(`    Fix: ${cmd}\n`);
			}
		}
	}
	process.stderr.write(`Summary: ${summary.ok} ok, ${summary.blocking} blocking, ${summary.degraded} degraded\n`);
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

const args = {
	json: {
		type: 'boolean' as const,
		description: 'Output as JSON envelope',
		default: false,
	},
} satisfies ArgsDef;

export const healthCommand = defineCommand({
	meta: {
		name: 'health',
		description: 'Check prerequisites and system health',
	},
	args,
	run() {
		const start = performance.now();
		const jsonMode = isJsonMode();
		const { checks, status } = runHealthChecks();
		const summary = computeSummary(checks);

		if (jsonMode) {
			const data = { status, checks, summary };
			const elapsed = Math.round(performance.now() - start);
			const envelope = { ok: true, data, meta: { tool: 'health', elapsed } };
			emitRaw(JSON.stringify(envelope), status === 'blocked' ? 2 : 0);
		}

		printTextOutput(checks, status, summary);
		if (status === 'blocked') process.exit(2);
	},
});
