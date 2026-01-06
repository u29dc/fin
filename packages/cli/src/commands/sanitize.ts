/**
 * sanitize - Discover and migrate descriptions.
 */

import { defineCommand } from 'citty';
import type { AssetAccountId, MigrationPlan, NameMappingConfig, RecategorizePlan } from 'core';
import { discoverDescriptions, discoverUnmappedDescriptions, executeMigration, executeRecategorize, isAssetAccountId, loadRules, planMigration, planRecategorize } from 'core';
import { initConfig } from 'core/config';

import { getDiscoverDb, getWritableDb } from '../db';
import { formatAmount } from '../format';
import { log } from '../logger';

let rulesConfig: NameMappingConfig | null = null;

async function getRulesConfig(): Promise<NameMappingConfig> {
	if (!rulesConfig) {
		initConfig();
		rulesConfig = await loadRules();
	}
	return rulesConfig;
}

function displayVerboseChanges(toUpdate: MigrationPlan['toUpdate'], limit = 50): void {
	if (toUpdate.length === 0) return;
	log('\nChanges:');
	for (const c of toUpdate.slice(0, limit)) {
		log(`  "${c.currentClean}" -> "${c.proposedClean}"`);
	}
	if (toUpdate.length > limit) {
		log(`  ... and ${toUpdate.length - limit} more`);
	}
}

function displayVerboseRecategorizations(toUpdate: RecategorizePlan['toUpdate'], limit = 50): void {
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

// ============================================================================
// discover
// ============================================================================

const discover = defineCommand({
	meta: { name: 'discover', description: 'Find description patterns' },
	args: {
		unmapped: { type: 'boolean', description: 'Show only unmapped descriptions' },
		min: { type: 'string', description: 'Minimum occurrences', default: '2' },
		account: { type: 'string', description: 'Filter by chart account ID' },
		db: { type: 'string', description: 'Database path' },
	},
	async run({ args }) {
		const unmappedOnly = args.unmapped ?? false;
		const minOccurrences = Number.parseInt(args.min ?? '2', 10);
		const chartAccountIdRaw = args.account;
		const dbPath = args.db;

		const db = getDiscoverDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const config = await getRulesConfig();

		const chartAccountId: AssetAccountId | undefined = chartAccountIdRaw && isAssetAccountId(chartAccountIdRaw) ? chartAccountIdRaw : undefined;
		const options = chartAccountId ? { minOccurrences, chartAccountId } : { minOccurrences };
		const results = unmappedOnly ? discoverUnmappedDescriptions(db, config, options) : discoverDescriptions(db, options);

		log(`Found ${results.length} unique descriptions${unmappedOnly ? ' (unmapped only)' : ''}:\n`);

		for (const r of results) {
			log(`"${r.rawDescription}"`);
			log(`  Occurrences: ${r.occurrences}, Total: ${formatAmount(r.totalAmountMinor)}`);
			log(`  Accounts: ${r.chartAccountIds.join(', ')}`);
			log(`  Range: ${r.firstSeen} to ${r.lastSeen}\n`);
		}
	},
});

// ============================================================================
// migrate
// ============================================================================

const migrate = defineCommand({
	meta: { name: 'migrate', description: 'Apply description mapping rules' },
	args: {
		'dry-run': { type: 'boolean', description: 'Preview changes without applying' },
		verbose: { type: 'boolean', description: 'Show detailed changes' },
		db: { type: 'string', description: 'Database path' },
	},
	async run({ args }) {
		const dryRun = args['dry-run'] ?? false;
		const verbose = args.verbose ?? false;
		const dbPath = args.db;

		const db = getWritableDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const config = await getRulesConfig();

		const plan = planMigration(db, config);

		log('Migration Plan:');
		log(`  To update: ${plan.toUpdate.length}`);
		log(`  Already clean: ${plan.alreadyClean}`);
		log(`  No matching rule: ${plan.noMatch}`);

		if (verbose) {
			displayVerboseChanges(plan.toUpdate);
		}

		if (dryRun) {
			log('\n[DRY RUN] No changes made.');
		} else if (plan.toUpdate.length === 0) {
			log('\nNo changes needed.');
		} else {
			const result = executeMigration(db, plan);
			log(`\nResult: ${result.updated} updated, ${result.skipped} skipped`);
			if (result.errors.length > 0) {
				log(`Errors: ${result.errors.length}`);
				for (const err of result.errors) {
					log(`  ${err.id}: ${err.error}`);
				}
			}
		}
	},
});

// ============================================================================
// recategorize
// ============================================================================

const recategorize = defineCommand({
	meta: { name: 'recategorize', description: 'Recategorize transactions based on rules' },
	args: {
		'dry-run': { type: 'boolean', description: 'Preview changes without applying' },
		verbose: { type: 'boolean', description: 'Show detailed changes' },
		db: { type: 'string', description: 'Database path' },
	},
	async run({ args }) {
		const dryRun = args['dry-run'] ?? false;
		const verbose = args.verbose ?? false;
		const dbPath = args.db;

		const db = getWritableDb(dbPath ? { options: new Map([['db', dbPath]]) } : undefined);
		const config = await getRulesConfig();

		const plan = planRecategorize(db, config);

		log('Recategorize Plan:');
		log(`  To update: ${plan.toUpdate.length}`);
		log(`  Already categorized: ${plan.alreadyCategorized}`);
		log(`  No better category found: ${plan.noMatch}`);

		if (verbose) {
			displayVerboseRecategorizations(plan.toUpdate);
		}

		if (dryRun) {
			log('\n[DRY RUN] No changes made.');
		} else if (plan.toUpdate.length === 0) {
			log('\nNo changes needed.');
		} else {
			const result = executeRecategorize(db, plan);
			log(`\nResult: ${result.updated} updated, ${result.skipped} skipped`);
			if (result.errors.length > 0) {
				log(`Errors: ${result.errors.length}`);
				for (const err of result.errors) {
					log(`  ${err.id}: ${err.error}`);
				}
			}
		}
	},
});

// ============================================================================
// sanitize (parent command)
// ============================================================================

export const sanitize = defineCommand({
	meta: { name: 'sanitize', description: 'Discover and apply description mappings' },
	subCommands: {
		discover,
		migrate,
		recategorize,
	},
});
