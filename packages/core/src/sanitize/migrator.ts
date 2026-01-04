import type { Database } from 'bun:sqlite';

import { mapToExpenseAccount } from '../db/category-mapping';
import { sanitizeDescription } from './matcher';
import type { MigrationCandidate, MigrationPlan, MigrationResult, NameMappingConfig, RecategorizeCandidate, RecategorizePlan, RecategorizeResult } from './types';

type JournalEntryRow = {
	id: string;
	raw_description: string;
	clean_description: string;
};

type UncategorizedPostingRow = {
	posting_id: string;
	journal_entry_id: string;
	description: string;
	raw_description: string | null;
	current_account_id: string;
};

/**
 * Plan a migration by identifying journal entries that need updating.
 *
 * Safety rules:
 * 1. If clean_description already equals proposed target, skip (already clean)
 * 2. If clean_description !== raw_description (manually edited), skip (preserve manual edits)
 * 3. Only update when raw matches pattern AND clean equals raw (untouched data)
 */
export function planMigration(db: Database, config: NameMappingConfig): MigrationPlan {
	const stmt = db.prepare(`
		SELECT id, raw_description, clean_description
		FROM journal_entries
		WHERE raw_description IS NOT NULL
	`);

	const rows = stmt.all() as JournalEntryRow[];

	const toUpdate: MigrationCandidate[] = [];
	let alreadyClean = 0;
	let noMatch = 0;

	for (const row of rows) {
		const result = sanitizeDescription(row.raw_description, config);

		if (result.matchedRule === null) {
			// No matching rule for this description
			noMatch++;
			continue;
		}

		// Check if name needs updating
		// Only update if clean_description equals raw_description (never been sanitized)
		const nameNeedsUpdate = row.clean_description !== result.cleanDescription && row.clean_description === row.raw_description;

		// Safety check: nothing to update
		if (!nameNeedsUpdate) {
			alreadyClean++;
			continue;
		}

		toUpdate.push({
			id: row.id,
			rawDescription: row.raw_description,
			currentClean: row.clean_description,
			proposedClean: result.cleanDescription,
		});
	}

	return { toUpdate, alreadyClean, noMatch };
}

/**
 * Execute migration with transaction safety.
 */
export function executeMigration(db: Database, plan: MigrationPlan, options: { dryRun?: boolean } = {}): MigrationResult {
	if (options.dryRun) {
		return {
			updated: plan.toUpdate.length,
			skipped: plan.alreadyClean + plan.noMatch,
			errors: [],
		};
	}

	const updateStmt = db.prepare(`
		UPDATE journal_entries
		SET description = ?, clean_description = ?, updated_at = datetime('now')
		WHERE id = ?
	`);

	let updated = 0;
	const errors: Array<{ id: string; error: string }> = [];

	db.transaction(() => {
		for (const candidate of plan.toUpdate) {
			try {
				updateStmt.run(candidate.proposedClean, candidate.proposedClean, candidate.id);
				updated++;
			} catch (err) {
				errors.push({
					id: candidate.id,
					error: err instanceof Error ? err.message : 'Unknown error',
				});
			}
		}
	})();

	return {
		updated,
		skipped: plan.alreadyClean + plan.noMatch,
		errors,
	};
}

/**
 * Plan a recategorization by identifying uncategorized postings that can be categorized.
 *
 * This finds postings with account_id = 'Expenses:Uncategorized' and attempts to
 * recategorize them based on the journal entry description and name mapping rules.
 */
export function planRecategorize(db: Database, config: NameMappingConfig): RecategorizePlan {
	const stmt = db.prepare(`
		SELECT
			p.id as posting_id,
			p.journal_entry_id,
			p.account_id as current_account_id,
			je.description,
			je.raw_description
		FROM postings p
		JOIN journal_entries je ON p.journal_entry_id = je.id
		WHERE p.account_id = 'Expenses:Uncategorized'
	`);

	const rows = stmt.all() as UncategorizedPostingRow[];

	const toUpdate: RecategorizeCandidate[] = [];
	let alreadyCategorized = 0;
	let noMatch = 0;

	for (const row of rows) {
		// Use raw_description if available, otherwise use description
		const descriptionToMatch = row.raw_description ?? row.description;

		// Get the sanitize result to find the category
		const result = sanitizeDescription(descriptionToMatch, config);
		const category = result.category;

		// Map category to expense account
		const proposedAccountId = mapToExpenseAccount(category, descriptionToMatch);

		// Skip if no better category found (still maps to Uncategorized)
		if (proposedAccountId === 'Expenses:Uncategorized') {
			noMatch++;
			continue;
		}

		// Skip if already categorized correctly (shouldn't happen, but safety check)
		if (proposedAccountId === row.current_account_id) {
			alreadyCategorized++;
			continue;
		}

		toUpdate.push({
			postingId: row.posting_id,
			journalEntryId: row.journal_entry_id,
			description: row.description,
			currentAccountId: row.current_account_id,
			proposedAccountId,
			category,
		});
	}

	return { toUpdate, alreadyCategorized, noMatch };
}

/**
 * Execute recategorization with transaction safety.
 */
export function executeRecategorize(db: Database, plan: RecategorizePlan, options: { dryRun?: boolean } = {}): RecategorizeResult {
	if (options.dryRun) {
		return {
			updated: plan.toUpdate.length,
			skipped: plan.alreadyCategorized + plan.noMatch,
			errors: [],
		};
	}

	const updateStmt = db.prepare(`
		UPDATE postings
		SET account_id = ?
		WHERE id = ?
	`);

	let updated = 0;
	const errors: Array<{ id: string; error: string }> = [];

	db.transaction(() => {
		for (const candidate of plan.toUpdate) {
			try {
				updateStmt.run(candidate.proposedAccountId, candidate.postingId);
				updated++;
			} catch (err) {
				errors.push({
					id: candidate.postingId,
					error: err instanceof Error ? err.message : 'Unknown error',
				});
			}
		}
	})();

	return {
		updated,
		skipped: plan.alreadyCategorized + plan.noMatch,
		errors,
	};
}
