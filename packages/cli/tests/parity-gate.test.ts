/**
 * Parity gate test: end-to-end agent orientation workflow.
 *
 * Demonstrates the complete agent loop using only new commands:
 *   tools -> health -> config show -> view accounts -> view balance -> report summary
 *
 * Each step produces a valid JSON envelope regardless of whether
 * the underlying data is available (empty DB returns empty arrays).
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

const TEMP_DIR = join(tmpdir(), `fin-parity-${Date.now()}-${Math.random().toString(36).slice(2)}`);
const DATA_DIR = join(TEMP_DIR, 'data');

mkdirSync(DATA_DIR, { recursive: true });

writeFileSync(
	join(DATA_DIR, 'fin.config.toml'),
	`
[[groups]]
id = "personal"
label = "Personal"
icon = "user"
tax_type = "income"
expense_reserve_months = 3

[[groups]]
id = "business"
label = "Business"
icon = "briefcase"
tax_type = "corp"
expense_reserve_months = 6

[[accounts]]
id = "Assets:Personal:Test"
group = "personal"
type = "asset"
provider = "monzo"
inbox_folder = "monzo-personal"

[[accounts]]
id = "Assets:Business:Test"
group = "business"
type = "asset"
provider = "wise"
inbox_folder = "wise-business"

[[banks]]
name = "monzo"
[banks.columns]
date = "Date"
description = "Description"
amount = "Amount"

[[banks]]
name = "wise"
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
// Helpers
// ---------------------------------------------------------------------------

const ENV = {
	FIN_HOME: TEMP_DIR,
};

/** Validate basic envelope structure */
function assertEnvelope(stdout: string, expectedTool: string): Record<string, unknown> {
	const parsed = JSON.parse(stdout) as Record<string, unknown>;
	expect(typeof parsed['ok']).toBe('boolean');
	expect(parsed['meta']).toBeDefined();
	const meta = parsed['meta'] as Record<string, unknown>;
	expect(meta['tool']).toBe(expectedTool);
	expect(typeof meta['elapsed']).toBe('number');
	return parsed;
}

// ---------------------------------------------------------------------------
// Parity gate chain: agent orientation workflow
// ---------------------------------------------------------------------------

describe('parity gate: agent orientation workflow', () => {
	test('1. tools --json -> returns tool catalog', async () => {
		const { stdout } = await run(['tools', '--json'], ENV);
		const parsed = assertEnvelope(stdout, 'tools');
		expect(parsed['ok']).toBe(true);
		const data = parsed['data'] as Record<string, unknown>;
		expect(Array.isArray(data['tools'])).toBe(true);
		expect((data['tools'] as unknown[]).length).toBeGreaterThan(0);
		expect(typeof data['version']).toBe('string');
	});

	test('2. health --json -> returns health status', async () => {
		const { stdout } = await run(['health', '--json'], ENV);
		const parsed = assertEnvelope(stdout, 'health');
		expect(parsed['ok']).toBe(true);
		const data = parsed['data'] as Record<string, unknown>;
		expect(data['status']).toMatch(/^(ready|degraded|blocked)$/);
		expect(Array.isArray(data['checks'])).toBe(true);
		expect(data['summary']).toBeDefined();
	});

	test('3. config show --json -> returns config with groups and accounts', async () => {
		const { stdout } = await run(['config', 'show', '--json'], ENV);
		const parsed = assertEnvelope(stdout, 'config.show');
		expect(parsed['ok']).toBe(true);
		const data = parsed['data'] as Record<string, unknown>;
		expect(data['groups']).toBeDefined();
		expect(data['accounts']).toBeDefined();
		expect(data['financial']).toBeDefined();
		const groups = data['groups'] as Array<Record<string, unknown>>;
		expect(groups.length).toBe(2);
	});

	test('4. view accounts --json -> valid envelope', async () => {
		const { stdout } = await run(['view', 'accounts', '--json'], ENV);
		const parsed = assertEnvelope(stdout, 'view.accounts');
		// May succeed (empty DB created) or fail (no DB) -- both are valid
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(Array.isArray(data['accounts'])).toBe(true);
			expect(typeof data['total']).toBe('number');
		}
	});

	test('5. view balance --json -> valid envelope', async () => {
		const { stdout } = await run(['view', 'balance', '--json'], ENV);
		const parsed = assertEnvelope(stdout, 'view.balance');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(typeof data['assets']).toBe('number');
			expect(typeof data['netWorth']).toBe('number');
		}
	});

	test('6. report summary --json -> valid envelope', async () => {
		const { stdout } = await run(['report', 'summary', '--json'], ENV);
		const parsed = assertEnvelope(stdout, 'report.summary');
		if (parsed['ok'] === true) {
			const data = parsed['data'] as Record<string, unknown>;
			expect(data['generatedAt']).toBeDefined();
			expect(data['currency']).toBeDefined();
			expect(Array.isArray(data['groups'])).toBe(true);
		}
	});
});

// ---------------------------------------------------------------------------
// Config validation chain (agent validates before querying)
// ---------------------------------------------------------------------------

describe('parity gate: config validation chain', () => {
	test('config validate --json -> config is valid', async () => {
		const { stdout, exitCode } = await run(['config', 'validate', '--json'], ENV);
		const parsed = assertEnvelope(stdout, 'config.validate');
		expect(parsed['ok']).toBe(true);
		const data = parsed['data'] as Record<string, unknown>;
		expect(data['valid']).toBe(true);
		expect(exitCode).toBe(0);
	});

	test('tools catalog has exactly 17 registered tools', async () => {
		const { stdout } = await run(['tools', '--json'], ENV);
		const parsed = JSON.parse(stdout) as Record<string, unknown>;
		const data = parsed['data'] as Record<string, unknown>;
		const tools = data['tools'] as Array<Record<string, unknown>>;
		expect(tools.length).toBe(17);
	});

	test('tools and health are infrastructure, not in catalog', async () => {
		const { stdout } = await run(['tools', '--json'], ENV);
		const parsed = JSON.parse(stdout) as Record<string, unknown>;
		const data = parsed['data'] as Record<string, unknown>;
		const tools = data['tools'] as Array<Record<string, unknown>>;
		const names = tools.map((t) => t['name'] as string);
		expect(names).not.toContain('tools');
		expect(names).not.toContain('health');
	});
});
