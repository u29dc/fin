import type { NameMappingConfig, NameMappingRule, SanitizeResult } from './types';

// Re-export rules for backward compatibility
export { NAME_MAPPING_CONFIG } from './rules';

type MatchMode = 'contains' | 'regex' | 'exact';

/**
 * Check exact match between value and pattern.
 */
function matchesExact(value: string, pattern: string, caseSensitive: boolean): boolean {
	const compareValue = caseSensitive ? value : value.toUpperCase();
	const comparePattern = caseSensitive ? pattern : pattern.toUpperCase();
	return compareValue === comparePattern;
}

/**
 * Check regex match between value and pattern.
 */
function matchesRegex(value: string, pattern: string, caseSensitive: boolean): boolean {
	try {
		const flags = caseSensitive ? '' : 'i';
		const regex = new RegExp(pattern, flags);
		return regex.test(value);
	} catch {
		return false;
	}
}

/**
 * Check contains match between value and pattern.
 */
function matchesContains(value: string, pattern: string, caseSensitive: boolean): boolean {
	const compareValue = caseSensitive ? value : value.toUpperCase();
	const comparePattern = caseSensitive ? pattern : pattern.toUpperCase();
	return compareValue.includes(comparePattern);
}

/**
 * Check if a single pattern matches the value.
 */
function matchesPattern(value: string, pattern: string, mode: MatchMode, caseSensitive: boolean): boolean {
	if (mode === 'exact') {
		return matchesExact(value, pattern, caseSensitive);
	}
	if (mode === 'regex') {
		return matchesRegex(value, pattern, caseSensitive);
	}
	return matchesContains(value, pattern, caseSensitive);
}

/**
 * Check if a value matches a single rule.
 */
function matchesRule(value: string, rule: NameMappingRule): boolean {
	const mode = rule.matchMode ?? 'contains';
	const caseSensitive = rule.caseSensitive ?? false;

	return rule.patterns.some((pattern) => matchesPattern(value, pattern, mode, caseSensitive));
}

/**
 * Apply name mapping rules to a raw description.
 * Pure function - no side effects.
 *
 * Safety: If raw already equals target, wasModified = false.
 */
export function sanitizeDescription(rawDescription: string, config: NameMappingConfig): SanitizeResult {
	const normalized = rawDescription.trim();

	for (const rule of config.rules) {
		if (matchesRule(normalized, rule)) {
			return {
				cleanDescription: rule.target,
				category: rule.category ?? null,
				matchedRule: rule,
				wasModified: normalized !== rule.target,
			};
		}
	}

	// No match - fall back to raw
	return {
		cleanDescription: config.fallbackToRaw ? normalized : rawDescription,
		category: null,
		matchedRule: null,
		wasModified: false,
	};
}

/**
 * Batch sanitize multiple descriptions.
 * Returns a map of raw -> result for deduplication.
 */
export function sanitizeBatch(rawDescriptions: string[], config: NameMappingConfig): Map<string, SanitizeResult> {
	const results = new Map<string, SanitizeResult>();

	for (const raw of rawDescriptions) {
		if (!results.has(raw)) {
			results.set(raw, sanitizeDescription(raw, config));
		}
	}

	return results;
}

/**
 * Get all unmapped descriptions from a batch.
 */
export function getUnmappedDescriptions(rawDescriptions: string[], config: NameMappingConfig): string[] {
	const unique = [...new Set(rawDescriptions)];
	return unique.filter((raw) => {
		const result = sanitizeDescription(raw, config);
		return result.matchedRule === null;
	});
}
