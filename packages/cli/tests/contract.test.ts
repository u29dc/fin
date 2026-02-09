/**
 * Contract tests for JSON envelope across commands.
 *
 * Verifies stdout purity and envelope shape for infrastructure commands.
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

// Write minimal valid config for health checks
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
// Contract tests
// ---------------------------------------------------------------------------

describe('JSON envelope contracts', () => {
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
