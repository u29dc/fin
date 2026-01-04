/**
 * ledger - Query journal entries with double-entry postings.
 */

import { getJournalEntries, getJournalEntryCount } from 'core';

import { getOption, parseArgs } from '../args';
import { getReadonlyDb } from '../db';
import { formatAmount, formatDate } from '../format';
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
		console.log(JSON.stringify(entries, null, 2));
		return;
	}

	if (format === 'tsv') {
		// Flat TSV output
		console.log('date\tdescription\taccount\tamount');
		for (const entry of entries) {
			for (const posting of entry.postings) {
				const date = entry.postedAt.slice(0, 10);
				const desc = entry.description.replace(/[\t\n]/g, ' ');
				console.log(`${date}\t${desc}\t${posting.accountId}\t${posting.amountMinor}`);
			}
		}
		return;
	}

	// Table format - custom rendering for ledger
	if (entries.length === 0) {
		console.log('No journal entries found.');
		return;
	}

	for (const entry of entries) {
		const date = formatDate(entry.postedAt.slice(0, 10));
		console.log(`${date}  ${entry.description}`);
		for (const posting of entry.postings) {
			console.log(formatPosting(posting.accountId, posting.amountMinor));
		}
		console.log('');
	}

	const total = getJournalEntryCount(db, accountId);
	console.log(`Showing ${entries.length} of ${total} entries`);
}
