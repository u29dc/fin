/**
 * ledger - Query journal entries with double-entry postings.
 */

import { getJournalEntries, getJournalEntryCount } from '@fin/core';

import { getOption, parseArgs } from '../args';
import { getReadonlyDb } from '../db';
import { formatAmount, formatDate } from '../format';
import { json, log } from '../logger';
import { parseFormat } from '../output';

function formatPosting(accountId: string, amountMinor: number): string {
	const sign = amountMinor >= 0 ? '+' : '';
	return `  ${accountId}: ${sign}${formatAmount(amountMinor)}`;
}

export function runLedger(args: string[]): void {
	const parsed = parseArgs(args);
	const format = parseFormat(getOption(parsed, 'format'));
	const accountId = getOption(parsed, 'account');
	const fromDate = getOption(parsed, 'from');
	const toDate = getOption(parsed, 'to');
	const limitStr = getOption(parsed, 'limit');
	const limit = limitStr ? Number.parseInt(limitStr, 10) : 50;

	const db = getReadonlyDb(parsed);

	type JournalOptions = Parameters<typeof getJournalEntries>[1];
	const options: JournalOptions = { limit };
	if (accountId) options.accountId = accountId;
	if (fromDate) options.startDate = fromDate;
	if (toDate) options.endDate = toDate;

	const entries = getJournalEntries(db, options);

	if (format === 'json') {
		json(entries);
		return;
	}

	if (format === 'tsv') {
		log(['date', 'description', 'account', 'amount'].join('\t'));
		for (const entry of entries) {
			for (const posting of entry.postings) {
				const date = entry.postedAt.slice(0, 10);
				const desc = entry.description.replace(/[\t\n]/g, ' ');
				log([date, desc, posting.accountId, posting.amountMinor].join('\t'));
			}
		}
		return;
	}

	// Table format - custom rendering for ledger
	if (entries.length === 0) {
		log('No journal entries found.');
		return;
	}

	for (const entry of entries) {
		const date = formatDate(entry.postedAt.slice(0, 10));
		log(`${date}  ${entry.description}`);
		for (const posting of entry.postings) {
			log(formatPosting(posting.accountId, posting.amountMinor));
		}
		log('');
	}

	const total = getJournalEntryCount(db, accountId);
	log(`Showing ${entries.length} of ${total} entries`);
}
