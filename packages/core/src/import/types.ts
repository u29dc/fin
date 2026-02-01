import type { AssetAccountId } from '../types/accounts';

export type { AssetAccountId };
export { getAssetAccountIds, getInboxFolderToChartId, isAssetAccountId } from '../types/accounts';

export type DetectedProvider = 'monzo' | 'wise' | 'vanguard' | 'freeagent';

export type DetectedFile = {
	path: string;
	provider: DetectedProvider;
	chartAccountId: AssetAccountId | null;
};

export type ArchiveFile = {
	path: string;
	provider: DetectedProvider;
	chartAccountId: AssetAccountId;
};

export type ParsedTransaction = {
	chartAccountId: AssetAccountId;
	postedAt: string;
	amountMinor: number;
	currency: string;
	rawDescription: string;
	counterparty: string | null;
	providerCategory: string | null;
	providerTxnId: string | null;
	balanceMinor: number | null;
	sourceFile: string;
};

export type ParseResult = {
	chartAccountId: AssetAccountId;
	transactions: ParsedTransaction[];
	hasBalances: boolean;
};
