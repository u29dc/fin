import { existsSync } from 'node:fs';
import { isAbsolute, join } from 'node:path';
import { getConfigDir, getRulesPath, isConfigInitialized } from '../config/index';
import { NAME_MAPPING_CONFIG } from './rules';
import type { NameMappingConfig } from './types';

let mergedRules: NameMappingConfig | null = null;
let externalRulesLoaded = false;

/**
 * Load external rules from the configured path (data/fin.rules.ts).
 * Returns null if path not configured or file doesn't exist.
 */
async function loadExternalRules(): Promise<NameMappingConfig | null> {
	if (!isConfigInitialized()) {
		return null;
	}

	const rulesPath = getRulesPath();
	if (!rulesPath) {
		return null;
	}

	// Resolve path relative to config directory if not absolute
	const configDir = getConfigDir();
	const resolvedPath = isAbsolute(rulesPath) ? rulesPath : configDir ? join(configDir, '..', rulesPath) : join(process.cwd(), rulesPath);

	if (!existsSync(resolvedPath)) {
		return null;
	}

	try {
		const externalModule = await import(/* @vite-ignore */ resolvedPath);
		if (externalModule.NAME_MAPPING_CONFIG) {
			return externalModule.NAME_MAPPING_CONFIG;
		}
		return null;
	} catch {
		// Silently return null if rules file is invalid
		return null;
	}
}

/**
 * Load and merge rules from generic (core) and external (data/fin.rules.ts) sources.
 * External rules take precedence (come first in the array).
 */
export async function loadRules(): Promise<NameMappingConfig> {
	if (mergedRules) {
		return mergedRules;
	}

	const externalConfig = await loadExternalRules();
	externalRulesLoaded = externalConfig !== null;

	if (externalConfig) {
		// External rules take precedence (come first)
		mergedRules = {
			rules: [...externalConfig.rules, ...NAME_MAPPING_CONFIG.rules],
			warnOnUnmapped: externalConfig.warnOnUnmapped ?? NAME_MAPPING_CONFIG.warnOnUnmapped,
			fallbackToRaw: externalConfig.fallbackToRaw ?? NAME_MAPPING_CONFIG.fallbackToRaw,
		};
	} else {
		mergedRules = NAME_MAPPING_CONFIG;
	}

	return mergedRules;
}

/**
 * Get the currently loaded rules.
 * Returns generic rules if loadRules() hasn't been called yet.
 */
export function getRules(): NameMappingConfig {
	return mergedRules ?? NAME_MAPPING_CONFIG;
}

/**
 * Reset the merged rules cache.
 * Useful for testing or when config changes.
 */
export function resetRulesCache(): void {
	mergedRules = null;
	externalRulesLoaded = false;
}

/**
 * Check if external rules are loaded (from data/fin.rules.ts).
 */
export function hasExternalRules(): boolean {
	return externalRulesLoaded;
}
