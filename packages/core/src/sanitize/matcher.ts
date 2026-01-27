import type { NameMappingConfig, NameMappingRule, SanitizeResult } from './types';

// Re-export rules for backward compatibility
export { NAME_MAPPING_CONFIG } from './rules';

type MatchMode = 'contains' | 'regex' | 'exact';

// Cache for pre-normalized uppercase patterns per rule
const normalizedPatternCache = new WeakMap<NameMappingRule, Map<string, string>>();

// Cache for compiled regex objects
const regexCache = new Map<string, RegExp | null>();

/**
 * Get or create normalized (uppercase) patterns for a rule.
 */
function getNormalizedPatterns(rule: NameMappingRule): Map<string, string> {
	let cache = normalizedPatternCache.get(rule);
	if (!cache) {
		cache = new Map();
		for (const pattern of rule.patterns) {
			cache.set(pattern, pattern.toUpperCase());
		}
		normalizedPatternCache.set(rule, cache);
	}
	return cache;
}

/**
 * Get or create a cached regex object.
 */
function getCachedRegex(pattern: string, flags: string): RegExp | null {
	const key = `${pattern}||${flags}`;
	if (regexCache.has(key)) {
		return regexCache.get(key) ?? null;
	}
	try {
		const regex = new RegExp(pattern, flags);
		regexCache.set(key, regex);
		return regex;
	} catch {
		regexCache.set(key, null);
		return null;
	}
}

/**
 * Check exact match between value and pattern.
 * valueUpper is pre-normalized when caseSensitive is false.
 */
function matchesExact(value: string, valueUpper: string, pattern: string, patternUpper: string, caseSensitive: boolean): boolean {
	return caseSensitive ? value === pattern : valueUpper === patternUpper;
}

/**
 * Check regex match between value and pattern.
 */
function matchesRegex(value: string, pattern: string, caseSensitive: boolean): boolean {
	const flags = caseSensitive ? '' : 'i';
	const regex = getCachedRegex(pattern, flags);
	return regex?.test(value) ?? false;
}

/**
 * Check contains match between value and pattern.
 * valueUpper is pre-normalized when caseSensitive is false.
 */
function matchesContains(value: string, valueUpper: string, pattern: string, patternUpper: string, caseSensitive: boolean): boolean {
	return caseSensitive ? value.includes(pattern) : valueUpper.includes(patternUpper);
}

/**
 * Check if a single pattern matches the value.
 */
function matchesPattern(value: string, valueUpper: string, pattern: string, patternUpper: string, mode: MatchMode, caseSensitive: boolean): boolean {
	if (mode === 'exact') {
		return matchesExact(value, valueUpper, pattern, patternUpper, caseSensitive);
	}
	if (mode === 'regex') {
		return matchesRegex(value, pattern, caseSensitive);
	}
	return matchesContains(value, valueUpper, pattern, patternUpper, caseSensitive);
}

/**
 * Check if a value matches a single rule.
 * valueUpper is pre-computed for case-insensitive matching.
 */
function matchesRule(value: string, valueUpper: string, rule: NameMappingRule): boolean {
	const mode = rule.matchMode ?? 'contains';
	const caseSensitive = rule.caseSensitive ?? false;
	const normalizedPatterns = getNormalizedPatterns(rule);

	return rule.patterns.some((pattern) => {
		const patternUpper = normalizedPatterns.get(pattern) ?? pattern;
		return matchesPattern(value, valueUpper, pattern, patternUpper, mode, caseSensitive);
	});
}

/**
 * Apply name mapping rules to a raw description.
 * Pure function - no side effects.
 *
 * Safety: If raw already equals target, wasModified = false.
 */
export function sanitizeDescription(rawDescription: string, config: NameMappingConfig): SanitizeResult {
	const normalized = rawDescription.trim();
	// Pre-compute uppercase once for all case-insensitive comparisons
	const normalizedUpper = normalized.toUpperCase();

	for (const rule of config.rules) {
		if (matchesRule(normalized, normalizedUpper, rule)) {
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
