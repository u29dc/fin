/**
 * Main command definition -- importable without side effects.
 *
 * Separated from index.ts so tests can import the command tree
 * and run commands in-process via citty's runCommand().
 */

import { defineCommand } from 'citty';
import { configCommand } from './commands/config';
import { healthCommand } from './commands/health';
import { help } from './commands/help';
import { importCommand } from './commands/import';
import { reportCommand } from './commands/report';
import { sanitizeCommand } from './commands/sanitize';
import { toolsCommand } from './commands/tools';
import { viewCommand } from './commands/view';

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
		health: healthCommand,
		config: configCommand,
		view: viewCommand,
		report: reportCommand,
		import: importCommand,
		sanitize: sanitizeCommand,
	},
});
