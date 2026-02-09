/**
 * `view` command group -- View accounts, transactions, ledger, balance sheet.
 *
 * All subcommands use defineToolCommand() and support --json envelope output.
 */

import { defineCommand } from 'citty';
import { viewAccountsCommand } from './accounts';
import { viewBalanceCommand } from './balance';
import { viewLedgerCommand } from './ledger';
import { viewTransactionsCommand } from './transactions';

export const viewCommand = defineCommand({
	meta: {
		name: 'view',
		description: 'View accounts, transactions, ledger, balance sheet',
	},
	subCommands: {
		accounts: viewAccountsCommand,
		transactions: viewTransactionsCommand,
		ledger: viewLedgerCommand,
		balance: viewBalanceCommand,
	},
});
