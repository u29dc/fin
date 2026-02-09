/**
 * Main command definition -- importable without side effects.
 *
 * Separated from index.ts so tests can import the command tree
 * and run commands in-process via citty's runCommand().
 */

import { defineCommand } from 'citty';
import { help } from './commands/help';
import { importCmd } from './commands/import';
import { report } from './commands/report';
import { sanitize } from './commands/sanitize';
import { toolsCommand } from './commands/tools';
import { view } from './commands/view';

/**
 * Root command - fin personal finance CLI
 */
export const main = defineCommand({
	meta: {
		name: 'fin',
		version: '0.0.1',
		description: 'Personal finance CLI',
	},
	subCommands: {
		help,
		tools: toolsCommand,
		view,
		report,
		import: importCmd,
		sanitize,
	},
});
