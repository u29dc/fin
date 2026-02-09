/**
 * `sanitize` command group -- Discover and apply description/category mappings.
 */

import { defineCommand } from 'citty';
import { sanitizeDiscoverCommand } from './discover';
import { sanitizeMigrateCommand } from './migrate';
import { sanitizeRecategorizeCommand } from './recategorize';

export const sanitizeCommand = defineCommand({
	meta: {
		name: 'sanitize',
		description: 'Discover and apply description mappings',
	},
	subCommands: {
		discover: sanitizeDiscoverCommand,
		migrate: sanitizeMigrateCommand,
		recategorize: sanitizeRecategorizeCommand,
	},
});
