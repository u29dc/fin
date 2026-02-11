/**
 * `edit` command group -- Edit transactions, accounts, and ledger data.
 *
 * All subcommands use defineToolCommand() and support --json envelope output.
 */

import { defineCommand } from 'citty';
import { editTransactionCommand } from './transaction';

export const editCommand = defineCommand({
	meta: {
		name: 'edit',
		description: 'Edit transactions, accounts, and ledger data',
	},
	subCommands: {
		transaction: editTransactionCommand,
	},
});
