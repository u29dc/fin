/**
 * help - Flattened help showing all commands and options.
 */

import { defineCommand } from 'citty';

import { log } from '../logger';

type ArgDef = {
	type: 'string' | 'boolean' | 'positional';
	description?: string;
	default?: string;
	required?: boolean;
};

type CommandInfo = {
	path: string;
	description: string;
	args: Record<string, ArgDef>;
};

type CategoryCommands = {
	category: string;
	commands: CommandInfo[];
};

// Define all commands statically for flattened help
// This avoids circular import issues and gives us full control over the output
const COMMAND_TREE: CategoryCommands[] = [
	{
		category: 'VIEW',
		commands: [
			{
				path: 'fin view accounts',
				description: 'List accounts with balances',
				args: {
					group: { type: 'string', description: 'Filter by group (personal, business, joint)' },
				},
			},
			{
				path: 'fin view transactions',
				description: 'Query transactions with filters',
				args: {
					account: { type: 'string', description: 'Filter by chart account ID' },
					group: { type: 'string', description: 'Filter by group' },
					from: { type: 'string', description: 'Start date (YYYY-MM-DD)' },
					to: { type: 'string', description: 'End date (YYYY-MM-DD)' },
					limit: { type: 'string', description: 'Max results', default: '50' },
				},
			},
			{
				path: 'fin view ledger',
				description: 'Query journal entries with postings',
				args: {
					account: { type: 'string', description: 'Filter by account ID' },
					from: { type: 'string', description: 'Start date (YYYY-MM-DD)' },
					to: { type: 'string', description: 'End date (YYYY-MM-DD)' },
					limit: { type: 'string', description: 'Max entries', default: '50' },
				},
			},
			{
				path: 'fin view balance',
				description: 'Display balance sheet',
				args: {
					'as-of': { type: 'string', description: 'Balance as of date (YYYY-MM-DD)' },
				},
			},
		],
	},
	{
		category: 'REPORT',
		commands: [
			{
				path: 'fin report cashflow',
				description: 'Monthly cashflow summary',
				args: {
					group: { type: 'string', description: 'Group ID', required: true },
					months: { type: 'string', description: 'Number of months', default: '12' },
					from: { type: 'string', description: 'Start month (YYYY-MM)' },
				},
			},
			{
				path: 'fin report health',
				description: 'Financial health metrics (balance - reserves)',
				args: {
					group: { type: 'string', description: 'Group ID', required: true },
					from: { type: 'string', description: 'Start date (YYYY-MM-DD)' },
					to: { type: 'string', description: 'End date (YYYY-MM-DD)' },
				},
			},
			{
				path: 'fin report runway',
				description: 'Months of cash remaining',
				args: {
					group: { type: 'string', description: 'Group ID', required: true },
					from: { type: 'string', description: 'Start date (YYYY-MM-DD)' },
					to: { type: 'string', description: 'End date (YYYY-MM-DD)' },
				},
			},
			{
				path: 'fin report reserves',
				description: 'Reserve breakdown (tax + expense)',
				args: {
					group: { type: 'string', description: 'Group ID', required: true },
					from: { type: 'string', description: 'Start date (YYYY-MM-DD)' },
					to: { type: 'string', description: 'End date (YYYY-MM-DD)' },
				},
			},
			{
				path: 'fin report categories breakdown',
				description: 'Total spending by category',
				args: {
					group: { type: 'string', description: 'Group ID', required: true },
					months: { type: 'string', description: 'Number of months', default: '3' },
					limit: { type: 'string', description: 'Max categories', default: '10' },
				},
			},
			{
				path: 'fin report categories median',
				description: 'Monthly median spending by category',
				args: {
					group: { type: 'string', description: 'Group ID', required: true },
					months: { type: 'string', description: 'Number of months', default: '6' },
					limit: { type: 'string', description: 'Max categories', default: '10' },
				},
			},
		],
	},
	{
		category: 'IMPORT',
		commands: [
			{
				path: 'fin import',
				description: 'Import transactions from inbox',
				args: {
					inbox: { type: 'string', description: 'Custom inbox directory' },
				},
			},
		],
	},
	{
		category: 'SANITIZE',
		commands: [
			{
				path: 'fin sanitize discover',
				description: 'Find description patterns',
				args: {
					unmapped: { type: 'boolean', description: 'Show only unmapped' },
					min: { type: 'string', description: 'Minimum occurrences', default: '2' },
					account: { type: 'string', description: 'Filter by account ID' },
				},
			},
			{
				path: 'fin sanitize migrate',
				description: 'Apply description mapping rules',
				args: {
					'dry-run': { type: 'boolean', description: 'Preview without applying' },
					verbose: { type: 'boolean', description: 'Show detailed changes' },
				},
			},
			{
				path: 'fin sanitize recategorize',
				description: 'Recategorize transactions based on rules',
				args: {
					'dry-run': { type: 'boolean', description: 'Preview without applying' },
					verbose: { type: 'boolean', description: 'Show detailed changes' },
				},
			},
		],
	},
];

function formatArg(name: string, def: ArgDef): string {
	let line = `  --${name}`;
	if (def.description) {
		line = line.padEnd(20) + def.description;
	}
	if (def.required) {
		line += ' [required]';
	}
	if (def.default) {
		line += ` (default: ${def.default})`;
	}
	return line;
}

function renderHelp(): void {
	log('fin - Personal finance CLI\n');

	for (const category of COMMAND_TREE) {
		log(category.category);

		for (const cmd of category.commands) {
			// Command line
			log(`  ${cmd.path.padEnd(35)} ${cmd.description}`);

			// Args (exclude global args like format, db)
			const argEntries = Object.entries(cmd.args);
			if (argEntries.length > 0) {
				for (const [name, def] of argEntries) {
					log(`    ${formatArg(name, def)}`);
				}
			}
			log('');
		}
	}

	log('GLOBAL OPTIONS');
	log('  --format        Output format: table, json, tsv');
	log('  --db            Database path');
	log('  --help          Show help\n');
}

export const help = defineCommand({
	meta: { name: 'help', description: 'Show all commands and options' },
	run() {
		renderHelp();
	},
});
