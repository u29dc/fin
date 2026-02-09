/**
 * `sanitize discover` -- Find description patterns.
 *
 * Queries unique raw_description values from journal entries to help build mapping rules.
 * With --unmapped, shows only descriptions not matched by current rules.
 */

import type { AssetAccountId } from '@fin/core';
import { discoverDescriptions, discoverUnmappedDescriptions, isAssetAccountId, loadRules } from '@fin/core';
import { getDiscoverDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount } from '../../format';
import { log } from '../../logger';
import { defineToolCommand } from '../../tool';

function renderTextOutput(
	results: Array<{ rawDescription: string; occurrences: number; totalAmountMinor: number; chartAccountIds: string[]; firstSeen: string; lastSeen: string }>,
	unmapped: boolean,
): void {
	log(`Found ${results.length} unique descriptions${unmapped ? ' (unmapped only)' : ''}:\n`);
	for (const r of results) {
		log(`"${r.rawDescription}"`);
		log(`  Occurrences: ${r.occurrences}, Total: ${formatAmount(r.totalAmountMinor)}`);
		log(`  Accounts: ${r.chartAccountIds.join(', ')}`);
		log(`  Range: ${r.firstSeen} to ${r.lastSeen}\n`);
	}
}

export const sanitizeDiscoverCommand = defineToolCommand(
	{
		name: 'sanitize.discover',
		command: 'fin sanitize discover',
		category: 'sanitize',
		outputFields: ['descriptions', 'count'],
		idempotent: true,
		rateLimit: null,
		example: 'fin sanitize discover --unmapped --json',
	},
	{
		meta: {
			name: 'discover',
			description: 'Find description patterns in journal entries',
		},
		args: {
			unmapped: { type: 'boolean' as const, description: 'Show only unmapped descriptions', default: false },
			min: { type: 'string' as const, description: 'Minimum occurrences', default: '2' },
			account: { type: 'string' as const, description: 'Filter by chart account ID' },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
		},
		async run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				const unmappedOnly = args.unmapped ?? false;
				const minOccurrences = Number.parseInt(args.min ?? '2', 10);
				const chartAccountIdRaw = args.account;

				const db = getDiscoverDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const config = await loadRules();

				const chartAccountId: AssetAccountId | undefined = chartAccountIdRaw && isAssetAccountId(chartAccountIdRaw) ? chartAccountIdRaw : undefined;
				const options = chartAccountId ? { minOccurrences, chartAccountId } : { minOccurrences };
				const results = unmappedOnly ? discoverUnmappedDescriptions(db, config, options) : discoverDescriptions(db, options);

				if (jsonMode) {
					ok('sanitize.discover', { descriptions: results, count: results.length }, start, { count: results.length });
				}

				renderTextOutput(results, unmappedOnly);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('sanitize.discover', 'DB_ERROR', `Failed to discover descriptions: ${message}`, 'Check database at data/fin.db and rules at data/fin.rules.ts', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
