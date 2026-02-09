/**
 * `view` command group -- View accounts, transactions, ledger, balance sheet.
 *
 * accounts and transactions are migrated to defineToolCommand().
 * ledger and balance remain as legacy commands until ENG-007.
 */

import { defineCommand } from 'citty';
import { balance, ledger } from '../view.legacy';
import { viewAccountsCommand } from './accounts';
import { viewTransactionsCommand } from './transactions';

export const viewCommand = defineCommand({
	meta: {
		name: 'view',
		description: 'View accounts, transactions, ledger, balance sheet',
	},
	subCommands: {
		accounts: viewAccountsCommand,
		transactions: viewTransactionsCommand,
		ledger,
		balance,
	},
});
