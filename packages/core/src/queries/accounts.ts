import type { Database } from 'bun:sqlite';

import { type AssetAccountId, getAssetAccountIds, isAssetAccountId } from '../types/accounts';

export type ChartAccountInfo = {
	id: string;
	name: string;
	accountType: string;
	parentId: string | null;
	currency: string;
	isPlaceholder: boolean;
	active: boolean;
};

type ChartAccountRow = {
	id: string;
	name: string;
	account_type: string;
	parent_id: string | null;
	currency: string;
	is_placeholder: number;
	active: number;
};

export function getChartAccounts(db: Database): ChartAccountInfo[] {
	const rows = db
		.query<ChartAccountRow, []>(
			`
			SELECT id, name, account_type, parent_id, currency, is_placeholder, active
			FROM chart_of_accounts
			WHERE active = 1
			ORDER BY id ASC;
			`,
		)
		.all();

	return rows.map((row: ChartAccountRow) => ({
		id: row.id,
		name: row.name,
		accountType: row.account_type,
		parentId: row.parent_id,
		currency: row.currency,
		isPlaceholder: row.is_placeholder === 1,
		active: row.active === 1,
	}));
}

export function getAssetAccounts(db: Database): ChartAccountInfo[] {
	const all = getChartAccounts(db);
	const assetIds = getAssetAccountIds();
	return all.filter((a) => assetIds.includes(a.id));
}

export { isAssetAccountId };
export type { AssetAccountId };
