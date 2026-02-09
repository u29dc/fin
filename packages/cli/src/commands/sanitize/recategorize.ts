/**
 * `sanitize recategorize` -- Recategorize transactions based on rules.
 *
 * Plans and executes recategorization of uncategorized postings using rules in data/fin.rules.ts.
 * With --dry-run, shows what would change without applying.
 */

import type { RecategorizePlan } from '@fin/core';
import { executeRecategorize, loadRules, planRecategorize } from '@fin/core';
import { getWritableDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { log } from '../../logger';
import { defineToolCommand } from '../../tool';

function renderVerboseRecategorizations(toUpdate: RecategorizePlan['toUpdate'], limit = 50): void {
	if (toUpdate.length === 0) return;
	log('\nRecategorizations:');
	for (const c of toUpdate.slice(0, limit)) {
		log(`  "${c.description}"`);
		log(`    ${c.currentAccountId} -> ${c.proposedAccountId}`);
		if (c.category) {
			log(`    (category: ${c.category})`);
		}
	}
	if (toUpdate.length > limit) {
		log(`  ... and ${toUpdate.length - limit} more`);
	}
}

function renderTextOutput(plan: RecategorizePlan, dryRun: boolean, verbose: boolean, result?: { updated: number; skipped: number; errors: Array<{ id: string; error: string }> }): void {
	log('Recategorize Plan:');
	log(`  To update: ${plan.toUpdate.length}`);
	log(`  Already categorized: ${plan.alreadyCategorized}`);
	log(`  No better category found: ${plan.noMatch}`);

	if (verbose) {
		renderVerboseRecategorizations(plan.toUpdate);
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

export const sanitizeRecategorizeCommand = defineToolCommand(
	{
		name: 'sanitize.recategorize',
		command: 'fin sanitize recategorize',
		category: 'sanitize',
		outputFields: ['plan', 'result'],
		idempotent: false,
		rateLimit: null,
		example: 'fin sanitize recategorize --dry-run --json',
	},
	{
		meta: {
			name: 'recategorize',
			description: 'Recategorize transactions based on rules',
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
				const plan = planRecategorize(db, config);

				const planData = {
					toUpdate: plan.toUpdate.length,
					alreadyCategorized: plan.alreadyCategorized,
					noMatch: plan.noMatch,
				};

				if (dryRun || plan.toUpdate.length === 0) {
					if (jsonMode) {
						ok('sanitize.recategorize', { plan: planData }, start, { count: plan.toUpdate.length });
					}
					renderTextOutput(plan, dryRun, verbose);
					return;
				}

				const result = executeRecategorize(db, plan);
				const resultData = {
					updated: result.updated,
					skipped: result.skipped,
					errors: result.errors,
				};

				if (jsonMode) {
					ok('sanitize.recategorize', { plan: planData, result: resultData }, start, { count: result.updated });
				}

				renderTextOutput(plan, dryRun, verbose, result);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('sanitize.recategorize', 'DB_ERROR', `Failed to recategorize: ${message}`, 'Check database at data/fin.db and rules at data/fin.rules.ts', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
