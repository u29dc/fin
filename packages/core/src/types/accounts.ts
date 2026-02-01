import {
	getAllAccountIds,
	getAssetAccountIds as getAssetIds,
	getGroupChartAccounts as getGroupAccounts,
	getGroupIds,
	getInboxFolderToChartId as getInboxMappings,
	isConfigInitialized,
} from '../config/index';

// String type for runtime flexibility
export type AssetAccountId = string;

export function isAssetAccountId(value: string): value is AssetAccountId {
	if (!isConfigInitialized()) {
		// Fallback for code paths before config init
		return value.startsWith('Assets:');
	}
	return getAllAccountIds().includes(value);
}

export type GroupId = string;

export function isGroupId(value: string): value is GroupId {
	if (!isConfigInitialized()) {
		return typeof value === 'string' && value.length > 0;
	}
	return getGroupIds().includes(value);
}

// Dynamic accessor functions
export function getAssetAccountIds(): AssetAccountId[] {
	return getAssetIds();
}

export function getGroupChartAccounts(): Record<string, AssetAccountId[]> {
	return getGroupAccounts();
}

export function getInboxFolderToChartId(): Record<string, AssetAccountId> {
	return getInboxMappings();
}
