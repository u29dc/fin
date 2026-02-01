import type { AssetAccountId } from '../types/accounts';

export type NameMappingRule = {
	patterns: string[];
	target: string;
	category?: string;
	caseSensitive?: boolean;
	matchMode?: 'contains' | 'regex' | 'exact';
};

export type NameMappingConfig = {
	rules: NameMappingRule[];
	warnOnUnmapped: boolean;
	fallbackToRaw: boolean;
};

export type SanitizeResult = {
	cleanDescription: string;
	category: string | null;
	matchedRule: NameMappingRule | null;
	wasModified: boolean;
};

export type DescriptionSummary = {
	rawDescription: string;
	occurrences: number;
	totalAmountMinor: number;
	chartAccountIds: string[];
	firstSeen: string;
	lastSeen: string;
};

export type DiscoveryOptions = {
	minOccurrences?: number;
	chartAccountId?: AssetAccountId;
	limit?: number;
	sortBy?: 'occurrences' | 'amount' | 'recent';
};

export type MigrationCandidate = {
	id: string;
	rawDescription: string;
	currentClean: string;
	proposedClean: string;
};

export type MigrationPlan = {
	toUpdate: MigrationCandidate[];
	alreadyClean: number;
	noMatch: number;
};

export type MigrationResult = {
	updated: number;
	skipped: number;
	errors: Array<{ id: string; error: string }>;
};

export type RecategorizeCandidate = {
	postingId: string;
	journalEntryId: string;
	description: string;
	currentAccountId: string;
	proposedAccountId: string;
	category: string | null;
};

export type RecategorizePlan = {
	toUpdate: RecategorizeCandidate[];
	alreadyCategorized: number;
	noMatch: number;
};

export type RecategorizeResult = {
	updated: number;
	skipped: number;
	errors: Array<{ id: string; error: string }>;
};
