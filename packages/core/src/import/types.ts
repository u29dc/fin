export type { AssetAccountId } from '../types/chart-account-ids';
export { getAssetAccountIds, getInboxFolderToChartId, isAssetAccountId } from '../types/chart-account-ids';

export type DetectedProvider = 'monzo' | 'wise' | 'vanguard' | 'freeagent';

export type DetectedFile = {
	path: string;
	provider: DetectedProvider;
	chartAccountId: AssetAccountId | null;
};

import type { AssetAccountId } from '../types/chart-account-ids';

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
