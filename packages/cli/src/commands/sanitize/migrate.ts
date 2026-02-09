/**
 * `sanitize migrate` -- Apply description mapping rules.
 *
 * Plans and executes description cleanup based on rules in data/fin.rules.ts.
 * With --dry-run, shows what would change without applying.
 */

import type { MigrationPlan } from '@fin/core';
import { executeMigration, loadRules, planMigration } from '@fin/core';
import { getWritableDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { log } from '../../logger';
import { defineToolCommand } from '../../tool';

function renderVerboseChanges(toUpdate: MigrationPlan['toUpdate'], limit = 50): void {
	if (toUpdate.length === 0) return;
	log('\nChanges:');
	for (const c of toUpdate.slice(0, limit)) {
		log(`  "${c.currentClean}" -> "${c.proposedClean}"`);
	}
	if (toUpdate.length > limit) {
		log(`  ... and ${toUpdate.length - limit} more`);
	}
}

function renderTextOutput(plan: MigrationPlan, dryRun: boolean, verbose: boolean, result?: { updated: number; skipped: number; errors: Array<{ id: string; error: string }> }): void {
	log('Migration Plan:');
	log(`  To update: ${plan.toUpdate.length}`);
	log(`  Already clean: ${plan.alreadyClean}`);
	log(`  No matching rule: ${plan.noMatch}`);

	if (verbose) {
		renderVerboseChanges(plan.toUpdate);
	}

	if (dryRun) {
		log('\n[DRY RUN] No changes made.');
	} else if (plan.toUpdate.length === 0) {
		log('\nNo changes needed.');
	} else if (result) {
		log(`\nResult: ${result.updated} updated, ${result.skipped} skipped`);
		if (result.errors.length > 0) {
			log(`Errors: ${result.errors.length}`);
			for (const err of result.errors) {
				log(`  ${err.id}: ${err.error}`);
			}
		}
	}
}

export const sanitizeMigrateCommand = defineToolCommand(
	{
		name: 'sanitize.migrate',
		command: 'fin sanitize migrate',
		category: 'sanitize',
		outputSchema: {
			plan: { type: 'object', description: 'Migration plan (toUpdate, alreadyClean, noMatch counts)' },
			result: { type: 'object', description: 'Execution result (updated, skipped, errors); absent in dry-run' },
		},
		idempotent: false,
		rateLimit: null,
		example: 'fin sanitize migrate --dry-run --json',
	},
	{
		meta: {
			name: 'migrate',
			description: 'Apply description mapping rules',
		},
		args: {
			'dry-run': { type: 'boolean' as const, description: 'Preview changes without applying', default: false },
			verbose: { type: 'boolean' as const, description: 'Show detailed changes', default: false },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
		},
		async run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				const dryRun = args['dry-run'] ?? false;
				const verbose = args.verbose ?? false;

				const db = getWritableDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const config = await loadRules();
				const plan = planMigration(db, config);

				const planData = {
					toUpdate: plan.toUpdate.length,
					alreadyClean: plan.alreadyClean,
					noMatch: plan.noMatch,
				};

				if (dryRun || plan.toUpdate.length === 0) {
					if (jsonMode) {
						ok('sanitize.migrate', { plan: planData }, start, { count: plan.toUpdate.length });
					}
					renderTextOutput(plan, dryRun, verbose);
					return;
				}

				const result = executeMigration(db, plan);
				const resultData = {
					updated: result.updated,
					skipped: result.skipped,
					errors: result.errors,
				};

				if (jsonMode) {
					ok('sanitize.migrate', { plan: planData, result: resultData }, start, { count: result.updated });
				}

				renderTextOutput(plan, dryRun, verbose, result);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('sanitize.migrate', 'DB_ERROR', `Failed to migrate descriptions: ${message}`, 'Check database at data/fin.db and rules at data/fin.rules.ts', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
