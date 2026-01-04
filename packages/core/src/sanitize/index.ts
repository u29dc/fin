export { discoverDescriptions, discoverUnmappedDescriptions } from './discovery';
export {
	getUnmappedDescriptions,
	NAME_MAPPING_CONFIG,
	sanitizeBatch,
	sanitizeDescription,
} from './matcher';
export { executeMigration, executeRecategorize, planMigration, planRecategorize } from './migrator';
export { getRules, hasExternalRules, loadRules, resetRulesCache } from './rules-loader';
export type {
	DescriptionSummary,
	DiscoveryOptions,
	MigrationCandidate,
	MigrationPlan,
	MigrationResult,
	NameMappingConfig,
	NameMappingRule,
	RecategorizeCandidate,
	RecategorizePlan,
	RecategorizeResult,
	SanitizeResult,
} from './types';
