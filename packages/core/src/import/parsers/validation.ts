import { getBankPreset, isConfigInitialized } from '../../config/index';

export type ColumnMapping = {
	date: string;
	time: string | undefined;
	description: string;
	amount: string;
	balance: string | undefined;
	transactionId: string | undefined;
	name: string | undefined;
	category: string | undefined;
};

// Default column mappings for each provider (used when config not initialized)
const DEFAULT_COLUMNS: Record<string, ColumnMapping> = {
	monzo: {
		date: 'Date',
		time: 'Time',
		description: 'Description',
		amount: 'Amount',
		balance: 'Balance',
		transactionId: 'Transaction ID',
		name: 'Name',
		category: 'Category',
	},
	wise: {
		date: 'Date',
		time: undefined,
		description: 'Description',
		amount: 'Amount',
		balance: 'Running Balance',
		transactionId: 'TransferWise ID',
		name: undefined,
		category: undefined,
	},
	vanguard: {
		date: 'Trade Date',
		time: undefined,
		description: 'Transaction Description',
		amount: 'Net Amount',
		balance: undefined,
		transactionId: undefined,
		name: undefined,
		category: undefined,
	},
};

/**
 * Get all configured column names for a provider.
 * Returns a map of semantic name -> actual column name from config.
 * Falls back to defaults if config not initialized (e.g., in tests).
 */
export function getColumnMapping(providerName: string): ColumnMapping {
	// Fall back to defaults if config not initialized
	if (!isConfigInitialized()) {
		const defaults = DEFAULT_COLUMNS[providerName];
		if (defaults) {
			return defaults;
		}
		throw new Error(`No default column mapping for provider: ${providerName}`);
	}

	const preset = getBankPreset(providerName);
	if (!preset) {
		// Fall back to defaults if no bank preset found
		const defaults = DEFAULT_COLUMNS[providerName];
		if (defaults) {
			return defaults;
		}
		throw new Error(`No bank preset found for provider: ${providerName}. Check [[banks]] in fin.config.toml`);
	}

	const cols = preset.columns;
	return {
		date: cols.date,
		time: cols.time,
		description: cols.description,
		amount: cols.amount,
		balance: cols.balance,
		transactionId: cols.transaction_id,
		name: cols.name,
		category: cols.category,
	};
}

/**
 * Get required columns for a provider from config.
 * Reads from [[banks]].columns in fin.config.toml, falls back to defaults if config not initialized.
 */
export function getRequiredColumns(providerName: string): string[] {
	const mapping = getColumnMapping(providerName);

	const required: string[] = [];

	// Core required columns
	if (mapping.date) required.push(mapping.date);
	if (mapping.description) required.push(mapping.description);
	if (mapping.amount) required.push(mapping.amount);

	return required;
}

/**
 * Validate that CSV headers contain all required columns for a provider.
 * Required columns are derived from the bank preset config.
 */
export function validateCsvHeaders(actualHeaders: string[], providerName: string): void {
	const required = getRequiredColumns(providerName);
	const headerSet = new Set(actualHeaders.map((h) => h.trim()));
	const missing = required.filter((col) => !headerSet.has(col));

	if (missing.length > 0) {
		throw new Error(`${providerName} CSV missing columns: ${missing.join(', ')}. Found: ${actualHeaders.join(', ')}. Check [[banks]] config in fin.config.toml`);
	}
}
