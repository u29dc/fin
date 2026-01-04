import type { NameMappingConfig } from './types';

/**
 * Generic name mapping rules for transaction description sanitization.
 *
 * This file contains public/generic rules that ship with the open source project.
 * User-specific rules should be placed in data/fin.rules.ts (not tracked in git).
 *
 * The rules-loader merges both files, with data/fin.rules.ts taking precedence.
 *
 * Patterns are matched case-insensitively by default using 'contains' mode.
 * Rule order matters - more specific patterns should come before general ones.
 */
export const NAME_MAPPING_CONFIG: NameMappingConfig = {
	warnOnUnmapped: true,
	fallbackToRaw: true,
	rules: [
		// Example: { patterns: ['AMAZON'], target: 'Amazon', category: 'Shopping' },
	],
};
