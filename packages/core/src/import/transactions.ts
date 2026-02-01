import type { NameMappingConfig } from '../sanitize';
import { NAME_MAPPING_CONFIG, sanitizeDescription } from '../sanitize';
import type { AssetAccountId, ParsedTransaction } from './types';

export type CanonicalTransaction = {
	id: string;
	chartAccountId: AssetAccountId;
	postedAt: string;
	amountMinor: number;
	currency: string;
	rawDescription: string;
	cleanDescription: string;
	counterparty: string | null;
	category: string | null;
	providerTxnId: string | null;
	balanceMinor: number | null;
	sourceFile: string;
};

export type CanonicalizationResult = {
	transactions: CanonicalTransaction[];
	unmappedDescriptions: string[];
};

export function canonicalize(parsed: ParsedTransaction[], config: NameMappingConfig = NAME_MAPPING_CONFIG): CanonicalizationResult {
	const unmappedSet = new Set<string>();

	const transactions = parsed.map((txn) => {
		let sanitized = sanitizeDescription(txn.rawDescription, config);

		// Fall back to counterparty matching for opaque descriptions (e.g. DD references)
		if (sanitized.matchedRule === null && txn.counterparty) {
			const fromCounterparty = sanitizeDescription(txn.counterparty, config);
			if (fromCounterparty.matchedRule !== null) {
				sanitized = fromCounterparty;
			}
		}

		if (sanitized.matchedRule === null && config.warnOnUnmapped) {
			unmappedSet.add(txn.rawDescription);
		}

		return {
			id: crypto.randomUUID(),
			chartAccountId: txn.chartAccountId,
			postedAt: txn.postedAt,
			amountMinor: txn.amountMinor,
			currency: txn.currency,
			rawDescription: txn.rawDescription,
			cleanDescription: sanitized.cleanDescription,
			counterparty: txn.counterparty,
			category: sanitized.category,
			providerTxnId: txn.providerTxnId,
			balanceMinor: txn.balanceMinor,
			sourceFile: txn.sourceFile,
		};
	});

	return {
		transactions,
		unmappedDescriptions: Array.from(unmappedSet),
	};
}
