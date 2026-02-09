/**
 * Registry drift test -- ensures every defineToolCommand() call
 * is reflected in the global toolRegistry[].
 *
 * Prevents adding a new command file that forgets to use
 * defineToolCommand() or that uses it but never gets imported
 * into the command tree.
 *
 * Strategy:
 * 1. Import main.ts to trigger all defineToolCommand() registrations
 * 2. Scan packages/cli/src/commands/ for files containing defineToolCommand(
 * 3. Verify counts match and no duplicates exist
 */

import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { Glob } from 'bun';

// Import main to trigger all defineToolCommand() registrations via the import chain
import '../src/main';
import { toolRegistry } from '../src/tool';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const COMMANDS_DIR = resolve(import.meta.dir, '../src/commands');

/** Scan command files for defineToolCommand() calls and extract tool names */
function scanCommandFiles(): { file: string; names: string[] }[] {
	const glob = new Glob('**/*.ts');
	const results: { file: string; names: string[] }[] = [];

	for (const path of glob.scanSync(COMMANDS_DIR)) {
		const fullPath = resolve(COMMANDS_DIR, path);
		const content = readFileSync(fullPath, 'utf-8');

		// Only care about files that actually call defineToolCommand(
		if (!content.includes('defineToolCommand(')) continue;

		// Skip files that only mention it in comments or imports
		const lines = content.split('\n');
		const callLines = lines.filter((line) => {
			const trimmed = line.trim();
			// Must contain the actual call, not just a comment or import
			return trimmed.includes('defineToolCommand(') && !trimmed.startsWith('//') && !trimmed.startsWith('*') && !trimmed.startsWith('import');
		});

		if (callLines.length === 0) continue;

		// Extract tool names from name: 'xxx' patterns near defineToolCommand calls
		const nameMatches = content.matchAll(/defineToolCommand\(\s*\{[^}]*?name:\s*'([^']+)'/gs);
		const names = [...nameMatches].map((m) => m[1] as string);

		if (names.length > 0) {
			results.push({ file: path, names });
		}
	}

	return results;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('registry drift', () => {
	test('toolRegistry has exactly 17 entries', () => {
		expect(toolRegistry.length).toBe(17);
	});

	test('no duplicate tool names in registry', () => {
		const names = toolRegistry.map((t) => t.name);
		const uniqueNames = new Set(names);
		expect(uniqueNames.size).toBe(names.length);
	});

	test('every defineToolCommand() file has its tools registered', () => {
		const scanned = scanCommandFiles();
		const registeredNames = new Set(toolRegistry.map((t) => t.name));

		const unregistered: string[] = [];
		for (const { file, names } of scanned) {
			for (const name of names) {
				if (!registeredNames.has(name)) {
					unregistered.push(`${file}: ${name}`);
				}
			}
		}

		expect(unregistered).toEqual([]);
	});

	test('every registered tool has a corresponding defineToolCommand() file', () => {
		const scanned = scanCommandFiles();
		const scannedNames = new Set(scanned.flatMap((s) => s.names));
		const registeredNames = toolRegistry.map((t) => t.name);

		const orphaned = registeredNames.filter((name) => !scannedNames.has(name));
		expect(orphaned).toEqual([]);
	});

	test('all 17 expected tool names are present', () => {
		const expected = [
			'config.show',
			'config.validate',
			'import',
			'report.audit',
			'report.cashflow',
			'report.categories',
			'report.health',
			'report.reserves',
			'report.runway',
			'report.summary',
			'sanitize.discover',
			'sanitize.migrate',
			'sanitize.recategorize',
			'view.accounts',
			'view.balance',
			'view.ledger',
			'view.transactions',
		];

		const actual = toolRegistry.map((t) => t.name).sort();
		expect(actual).toEqual(expected);
	});
});
