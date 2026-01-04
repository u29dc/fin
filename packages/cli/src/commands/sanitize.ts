/**
 * sanitize - Discover and migrate descriptions.
 */

import type { AssetAccountId, MigrationPlan, NameMappingConfig, RecategorizePlan } from 'core';
import { discoverDescriptions, discoverUnmappedDescriptions, executeMigration, executeRecategorize, isAssetAccountId, loadRules, planMigration, planRecategorize } from 'core';
import { initConfig } from 'core/config';

import { getOption, getOptionAsNumberOrDefault, hasFlag, parseArgs } from '../args';
import { getDiscoverDb, getWritableDb } from '../db';
import { formatAmount } from '../format';

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
	console.log('\nChanges:');
	for (const c of toUpdate.slice(0, limit)) {
		console.log(`  "${c.currentClean}" -> "${c.proposedClean}"`);
	}
	if (toUpdate.length > limit) {
		console.log(`  ... and ${toUpdate.length - limit} more`);
	}
}

function displayVerboseRecategorizations(toUpdate: RecategorizePlan['toUpdate'], limit = 50): void {
	if (toUpdate.length === 0) return;
	console.log('\nRecategorizations:');
	for (const c of toUpdate.slice(0, limit)) {
		console.log(`  "${c.description}"`);
		console.log(`    ${c.currentAccountId} -> ${c.proposedAccountId}`);
		if (c.category) {
			console.log(`    (category: ${c.category})`);
		}
	}
	if (toUpdate.length > limit) {
		console.log(`  ... and ${toUpdate.length - limit} more`);
	}
}

export async function runSanitize(args: string[]): Promise<void> {
	const parsed = parseArgs(args);
	const subcommand = parsed.positional[0];

	if (!subcommand || !['discover', 'migrate', 'recategorize'].includes(subcommand)) {
		console.error('Usage: fin sanitize <discover|migrate|recategorize> [options]');
		process.exit(1);
	}

	if (subcommand === 'discover') {
		await runDiscover(args.slice(1));
	} else if (subcommand === 'migrate') {
		await runMigrate(args.slice(1));
	} else if (subcommand === 'recategorize') {
		await runRecategorize(args.slice(1));
	}
}

async function runDiscover(args: string[]): Promise<void> {
	const parsed = parseArgs(args);
	const unmappedOnly = hasFlag(parsed, 'unmapped');
	const minOccurrences = getOptionAsNumberOrDefault(parsed, 'min', 2);
	const chartAccountIdRaw = getOption(parsed, 'account');

	const db = getDiscoverDb(parsed);
	const config = await getRulesConfig();

	const chartAccountId: AssetAccountId | undefined = chartAccountIdRaw && isAssetAccountId(chartAccountIdRaw) ? chartAccountIdRaw : undefined;
	const options = chartAccountId ? { minOccurrences, chartAccountId } : { minOccurrences };
	const results = unmappedOnly ? discoverUnmappedDescriptions(db, config, options) : discoverDescriptions(db, options);

	console.log(`Found ${results.length} unique descriptions${unmappedOnly ? ' (unmapped only)' : ''}:\n`);

	for (const r of results) {
		console.log(`"${r.rawDescription}"`);
		console.log(`  Occurrences: ${r.occurrences}, Total: ${formatAmount(r.totalAmountMinor)}`);
		console.log(`  Accounts: ${r.chartAccountIds.join(', ')}`);
		console.log(`  Range: ${r.firstSeen} to ${r.lastSeen}\n`);
	}
}

async function runMigrate(args: string[]): Promise<void> {
	const parsed = parseArgs(args);
	const dryRun = hasFlag(parsed, 'dry-run');
	const verbose = hasFlag(parsed, 'verbose');

	const db = getWritableDb(parsed);
	const config = await getRulesConfig();

	const plan = planMigration(db, config);

	console.log('Migration Plan:');
	console.log(`  To update: ${plan.toUpdate.length}`);
	console.log(`  Already clean: ${plan.alreadyClean}`);
	console.log(`  No matching rule: ${plan.noMatch}`);

	if (verbose) {
		displayVerboseChanges(plan.toUpdate);
	}

	if (dryRun) {
		console.log('\n[DRY RUN] No changes made.');
	} else if (plan.toUpdate.length === 0) {
		console.log('\nNo changes needed.');
	} else {
		const result = executeMigration(db, plan);
		console.log(`\nResult: ${result.updated} updated, ${result.skipped} skipped`);
		if (result.errors.length > 0) {
			console.log(`Errors: ${result.errors.length}`);
			for (const err of result.errors) {
				console.log(`  ${err.id}: ${err.error}`);
			}
		}
	}
}

async function runRecategorize(args: string[]): Promise<void> {
	const parsed = parseArgs(args);
	const dryRun = hasFlag(parsed, 'dry-run');
	const verbose = hasFlag(parsed, 'verbose');

	const db = getWritableDb(parsed);
	const config = await getRulesConfig();

	const plan = planRecategorize(db, config);

	console.log('Recategorize Plan:');
	console.log(`  To update: ${plan.toUpdate.length}`);
	console.log(`  Already categorized: ${plan.alreadyCategorized}`);
	console.log(`  No better category found: ${plan.noMatch}`);

	if (verbose) {
		displayVerboseRecategorizations(plan.toUpdate);
	}

	if (dryRun) {
		console.log('\n[DRY RUN] No changes made.');
	} else if (plan.toUpdate.length === 0) {
		console.log('\nNo changes needed.');
	} else {
		const result = executeRecategorize(db, plan);
		console.log(`\nResult: ${result.updated} updated, ${result.skipped} skipped`);
		if (result.errors.length > 0) {
			console.log(`Errors: ${result.errors.length}`);
			for (const err of result.errors) {
				console.log(`  ${err.id}: ${err.error}`);
			}
		}
	}
}
