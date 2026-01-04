export {
	findMonorepoRoot,
	getConfig,
	getConfigDir,
	getConfigPath,
	initConfig,
	isConfigInitialized,
	loadConfig,
	resetConfig,
} from './loader';

export {
	type Account,
	type BankColumns,
	type BankPreset,
	type FinConfig,
	FinConfigSchema,
	type GroupId,
	type GroupMetadata as GroupMetadataConfig,
} from './schema';

import { getConfig } from './loader';
import type { Account, BankPreset } from './schema';

// Group metadata type with runtime-friendly property names
export type GroupMetadata = {
	id: string;
	label: string;
	icon: string;
	taxType: 'corp' | 'income' | 'none';
	expenseReserveMonths: number;
};

// Default group metadata for backwards compatibility
const DEFAULT_GROUP_METADATA: Record<string, GroupMetadata> = {
	personal: { id: 'personal', label: 'Personal', icon: 'user', taxType: 'income', expenseReserveMonths: 3 },
	joint: { id: 'joint', label: 'Joint', icon: 'heart', taxType: 'none', expenseReserveMonths: 3 },
	business: { id: 'business', label: 'Business', icon: 'briefcase', taxType: 'corp', expenseReserveMonths: 1 },
};

// Account accessors
export function getAccountById(id: string): Account | undefined {
	return getConfig().accounts.find((a) => a.id === id);
}

export function getAccountsByGroup(group: string): Account[] {
	return getConfig().accounts.filter((a) => a.group === group);
}

export function getAccountIdsByGroup(group: string): string[] {
	return getConfig()
		.accounts.filter((a) => a.group === group)
		.map((a) => a.id);
}

export function getFirstAccountIdByGroup(group: string): string | undefined {
	const account = getConfig().accounts.find((a) => a.group === group);
	return account?.id;
}

/**
 * Returns asset account IDs considered "liquid" (readily accessible).
 * Excludes investment accounts (provider: vanguard).
 */
export function getLiquidAccountIds(): string[] {
	return getConfig()
		.accounts.filter((a) => a.type === 'asset' && a.provider !== 'vanguard')
		.map((a) => a.id);
}

export function getAccountsByProvider(provider: string): Account[] {
	return getConfig().accounts.filter((a) => a.provider === provider);
}

export function getAllAccountIds(): string[] {
	return getConfig().accounts.map((a) => a.id);
}

export function getAssetAccountIds(): string[] {
	return getConfig()
		.accounts.filter((a) => a.type === 'asset')
		.map((a) => a.id);
}

export function getGroupIds(): string[] {
	const groups = new Set(getConfig().accounts.map((a) => a.group));
	return [...groups];
}

export function getGroupChartAccounts(): Record<string, string[]> {
	const groups: Record<string, string[]> = {};
	// Initialize from actual groups in config (not hardcoded)
	for (const groupId of getGroupIds()) {
		groups[groupId] = [];
	}
	for (const account of getConfig().accounts) {
		const groupAccounts = groups[account.group];
		if (groupAccounts) {
			groupAccounts.push(account.id);
		}
	}
	return groups;
}

/**
 * Get metadata for a specific group.
 * Checks explicit config first, then falls back to defaults for known groups.
 * Unknown groups get sensible defaults.
 */
export function getGroupMetadata(groupId: string): GroupMetadata {
	const config = getConfig();
	const explicit = config.groups?.find((g) => g.id === groupId);
	if (explicit) {
		return {
			id: explicit.id,
			label: explicit.label,
			icon: explicit.icon,
			taxType: explicit.tax_type,
			expenseReserveMonths: explicit.expense_reserve_months,
		};
	}
	// Fallback for backwards compatibility
	const def = DEFAULT_GROUP_METADATA[groupId];
	if (def) {
		return def;
	}
	// Unknown group: use sensible defaults
	return {
		id: groupId,
		label: groupId.charAt(0).toUpperCase() + groupId.slice(1),
		icon: 'wallet',
		taxType: 'none',
		expenseReserveMonths: 3,
	};
}

/**
 * Get metadata for all groups defined in config.
 */
export function getAllGroupMetadata(): GroupMetadata[] {
	return getGroupIds().map((id) => getGroupMetadata(id));
}

// Bank preset accessors
export function getBankPreset(name: string): BankPreset | undefined {
	return getConfig().banks.find((b) => b.name === name);
}

export function getAllBankPresets(): BankPreset[] {
	return getConfig().banks;
}

// Inbox mapping accessor - derives from account.inbox_folder
export function getInboxFolderToChartId(): Record<string, string> {
	const mapping: Record<string, string> = {};
	for (const account of getConfig().accounts) {
		if (account.inbox_folder) {
			mapping[account.inbox_folder] = account.id;
		}
	}
	return mapping;
}

// Financial config accessors
export function getFinancialConfig() {
	return getConfig().financial;
}

// Provider mapping from account
export function getProviderForAccount(chartAccountId: string): string | null {
	const account = getAccountById(chartAccountId);
	return account?.provider ?? null;
}

// Sanitization config
export function getSanitizationConfig() {
	return getConfig().sanitization;
}

export function getRulesPath(): string | undefined {
	return getConfig().sanitization?.rules;
}
