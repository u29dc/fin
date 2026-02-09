/**
 * Contract tests for JSON envelope across all commands.
 *
 * Verifies stdout purity and envelope shape for all 17 tool commands
 * plus 2 infrastructure commands (tools, health).
 *
 * Each command is tested with --json to ensure:
 * 1. stdout contains exactly one JSON line
 * 2. Envelope has { ok, data|error, meta } structure
 * 3. meta.tool matches expected tool name
 * 4. meta.elapsed is a non-negative number
 *
 * Uses in-process execution via harness for speed.
 */

import { afterAll, describe, expect, test } from 'bun:test';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { run } from './harness';

// ---------------------------------------------------------------------------
// Fixture setup
// ---------------------------------------------------------------------------

const TEMP_DIR = join(tmpdir(), `fin-contract-${Date.now()}-${Math.random().toString(36).slice(2)}`);
const DATA_DIR = join(TEMP_DIR, 'data');

mkdirSync(DATA_DIR, { recursive: true });

// Write minimal valid config
writeFileSync(
	join(DATA_DIR, 'fin.config.toml'),
	`
[[groups]]
id = "personal"
label = "Personal"
icon = "user"
tax_type = "income"
expense_reserve_months = 3

[[accounts]]
id = "Assets:Personal:Test"
group = "personal"
type = "asset"
provider = "monzo"
inbox_folder = "monzo-personal"

[[banks]]
name = "monzo"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"

[financial]
corp_tax_rate = 0.25
vat_rate = 0.20
personal_income_tax_rate = 0.20
joint_share_you = 0.50
expense_reserve_months = 3
trailing_expense_window_months = 6
fixed_monthly_personal_outflow_minor = 0
burn_rate_method = "median"
burn_rate_exclude_accounts = []
[financial.personal_dividend_tax]
allowance_minor = 50000
basic_rate = 0.0875
higher_rate = 0.3375
[financial.scenario]
lookback_months = 6
salary_dividend_split_minor = 0
dividends_monthly_minor = 0
salary_monthly_minor = 0
joint_expenses_monthly_minor = 0
[financial.investment_projection_annual_returns]
low = 0.04
mid = 0.07
high = 0.10
`,
);

// Create inbox directory for health check
mkdirSync(join(TEMP_DIR, 'imports', 'inbox'), { recursive: true });

afterAll(() => {
	try {
		rmSync(TEMP_DIR, { recursive: true, force: true });
	} catch {
		// ignore cleanup errors
	}
});

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

const ENV = {
	FIN_HOME: TEMP_DIR,
};

/** Validate JSON envelope structure and return parsed object */
function assertValidEnvelope(stdout: string, expectedTool: string): Record<string, unknown> {
	const parsed = JSON.parse(stdout) as Record<string, unknown>;

	// Must have ok field
	expect(typeof parsed['ok']).toBe('boolean');

	// Must have meta with tool and elapsed
	expect(parsed['meta']).toBeDefined();
	const meta = parsed['meta'] as Record<string, unknown>;
	expect(meta['tool']).toBe(expectedTool);
	expect(typeof meta['elapsed']).toBe('number');
	expect(meta['elapsed'] as number).toBeGreaterThanOrEqual(0);

	if (parsed['ok'] === true) {
		// Success: must have data
		expect(parsed['data']).toBeDefined();
	} else {
		// Error: must have error with code, message, hint
		expect(parsed['error']).toBeDefined();
		const error = parsed['error'] as Record<string, unknown>;
		expect(typeof error['code']).toBe('string');
		expect(typeof error['message']).toBe('string');
		expect(typeof error['hint']).toBe('string');
	}

	return parsed;
}

// ---------------------------------------------------------------------------
// Infrastructure commands (tools, health)
// ---------------------------------------------------------------------------

describe('infrastructure commands', () => {
	test('tools --json', async () => {
		const { stdout } = await run(['tools', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'tools');
		expect(parsed['ok']).toBe(true);
		const data = parsed['data'] as Record<string, unknown>;
		expect(Array.isArray(data['tools'])).toBe(true);
		expect(typeof data['version']).toBe('string');
	});

	test('tools <name> --json (valid tool)', async () => {
		const { stdout } = await run(['tools', 'config.show', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'tools');
		expect(parsed['ok']).toBe(true);
		const data = parsed['data'] as Record<string, unknown>;
		expect(data['tool']).toBeDefined();
	});

	test('tools <name> --json (not found)', async () => {
		const { stdout, exitCode } = await run(['tools', 'nonexistent.tool', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'tools');
		expect(parsed['ok']).toBe(false);
		const error = parsed['error'] as Record<string, unknown>;
		expect(error['code']).toBe('NOT_FOUND');
		expect(exitCode).toBe(1);
	});

	test('health --json', async () => {
		const { stdout } = await run(['health', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'health');
		expect(parsed['ok']).toBe(true);
		const data = parsed['data'] as Record<string, unknown>;
		expect(data['status']).toBeDefined();
		expect(Array.isArray(data['checks'])).toBe(true);
		expect(data['summary']).toBeDefined();
	});

	test('health --json has correct check IDs', async () => {
		const { stdout } = await run(['health', '--json'], ENV);
		const parsed = JSON.parse(stdout) as Record<string, unknown>;
		const data = parsed['data'] as Record<string, unknown>;
		const checks = data['checks'] as Array<Record<string, unknown>>;
		const checkIds = checks.map((c) => c['id']);
		expect(checkIds).toContain('config');
		expect(checkIds).toContain('config_valid');
		expect(checkIds).toContain('database');
		expect(checkIds).toContain('rules');
		expect(checkIds).toContain('inbox');
	});
});

// ---------------------------------------------------------------------------
// Config commands (config present, no DB needed)
// ---------------------------------------------------------------------------

describe('config commands', () => {
	test('config.show --json', async () => {
		const { stdout } = await run(['config', 'show', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'config.show');
		expect(parsed['ok']).toBe(true);
		const data = parsed['data'] as Record<string, unknown>;
		expect(data['groups']).toBeDefined();
		expect(data['accounts']).toBeDefined();
		expect(data['financial']).toBeDefined();
		expect(data['configPath']).toBeDefined();
	});

	test('config.validate --json', async () => {
		const { stdout } = await run(['config', 'validate', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'config.validate');
		expect(parsed['ok']).toBe(true);
		const data = parsed['data'] as Record<string, unknown>;
		expect(data['valid']).toBe(true);
		expect(Array.isArray(data['errors'])).toBe(true);
	});
});

// ---------------------------------------------------------------------------
// View commands (DB-dependent -- verify envelope shape)
// ---------------------------------------------------------------------------

describe('view commands', () => {
	test('view.accounts --json', async () => {
		const { stdout } = await run(['view', 'accounts', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'view.accounts');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['accounts'])).toBe(true);
			expect(typeof data['total']).toBe('number');
		}
	});

	test('view.transactions --json', async () => {
		const { stdout } = await run(['view', 'transactions', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'view.transactions');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['transactions'])).toBe(true);
			expect(typeof data['count']).toBe('number');
		}
	});

	test('view.ledger --json', async () => {
		const { stdout } = await run(['view', 'ledger', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'view.ledger');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['entries'])).toBe(true);
			expect(typeof data['count']).toBe('number');
			expect(typeof data['total']).toBe('number');
		}
	});

	test('view.balance --json', async () => {
		const { stdout } = await run(['view', 'balance', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'view.balance');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(typeof data['assets']).toBe('number');
			expect(typeof data['liabilities']).toBe('number');
			expect(typeof data['equity']).toBe('number');
			expect(typeof data['netWorth']).toBe('number');
			expect(typeof data['netIncome']).toBe('number');
		}
	});
});

// ---------------------------------------------------------------------------
// Report commands (DB-dependent -- verify envelope shape)
// ---------------------------------------------------------------------------

describe('report commands', () => {
	test('report.cashflow --json', async () => {
		const { stdout } = await run(['report', 'cashflow', '--group=personal', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'report.cashflow');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['series'])).toBe(true);
			expect(data['totals']).toBeDefined();
		}
	});

	test('report.health --json', async () => {
		const { stdout } = await run(['report', 'health', '--group=personal', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'report.health');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['series'])).toBe(true);
			expect(data['latest']).toBeDefined();
		}
	});

	test('report.runway --json', async () => {
		const { stdout } = await run(['report', 'runway', '--group=personal', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'report.runway');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['series'])).toBe(true);
			expect(data['latest']).toBeDefined();
		}
	});

	test('report.reserves --json', async () => {
		const { stdout } = await run(['report', 'reserves', '--group=personal', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'report.reserves');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['series'])).toBe(true);
			expect(data['latest']).toBeDefined();
		}
	});

	test('report.categories --json', async () => {
		const { stdout } = await run(['report', 'categories', '--group=personal', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'report.categories');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['categories'])).toBe(true);
			expect(typeof data['total']).toBe('number');
		}
	});

	test('report.audit --json', async () => {
		const { stdout } = await run(['report', 'audit', '--account=Expenses:Personal:Test', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'report.audit');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['payees'])).toBe(true);
			expect(typeof data['total']).toBe('number');
		}
	});

	test('report.summary --json', async () => {
		const { stdout } = await run(['report', 'summary', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'report.summary');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(data['generatedAt']).toBeDefined();
			expect(data['currency']).toBeDefined();
			expect(Array.isArray(data['groups'])).toBe(true);
		}
	});
});

// ---------------------------------------------------------------------------
// Import command (verify envelope shape)
// ---------------------------------------------------------------------------

describe('import command', () => {
	test('import --json', async () => {
		const { stdout } = await run(['import', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'import');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['processedFiles'])).toBe(true);
			expect(typeof data['totalTransactions']).toBe('number');
			expect(typeof data['journalEntriesCreated']).toBe('number');
		}
	});
});

// ---------------------------------------------------------------------------
// Sanitize commands (verify envelope shape)
// ---------------------------------------------------------------------------

describe('sanitize commands', () => {
	test('sanitize.discover --json', async () => {
		const { stdout } = await run(['sanitize', 'discover', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'sanitize.discover');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['descriptions'])).toBe(true);
			expect(typeof data['count']).toBe('number');
		}
	});

	test('sanitize.migrate --json', async () => {
		const { stdout } = await run(['sanitize', 'migrate', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'sanitize.migrate');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(data['plan']).toBeDefined();
		}
	});

	test('sanitize.recategorize --json', async () => {
		const { stdout } = await run(['sanitize', 'recategorize', '--json'], ENV);
		const parsed = assertValidEnvelope(stdout, 'sanitize.recategorize');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(data['plan']).toBeDefined();
		}
	});
});
