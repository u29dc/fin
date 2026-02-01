#!/usr/bin/env bun
/**
 * fin - Personal finance CLI
 *
 * Commands:
 *   view      View accounts, transactions, ledger, balance sheet
 *   report    Financial analytics and reports
 *   import    Import transactions from inbox
 *   sanitize  Discover and apply description mappings
 */

import { initConfig } from '@fin/core/config';
import { defineCommand, runMain } from 'citty';

import { help } from './commands/help';
import { importCmd } from './commands/import';
import { report } from './commands/report';
import { sanitize } from './commands/sanitize';
import { view } from './commands/view';

const main = defineCommand({
	meta: {
		name: 'fin',
		version: '0.0.1',
		description: 'Personal finance CLI',
	},
	subCommands: {
		help,
		view,
		report,
		import: importCmd,
		sanitize,
	},
	setup() {
		initConfig();
	},
});

runMain(main);
