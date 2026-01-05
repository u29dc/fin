#!/usr/bin/env bun
/**
 * CLI entry point and command router.
 */

import { initConfig } from 'core/config';
import { runAccounts } from './commands/accounts';
import { runBalanceSheet } from './commands/balance-sheet';
import { runCashflow } from './commands/cashflow';
import { runCategories } from './commands/categories';
import { runHealth } from './commands/health';
import { runImport } from './commands/import';
import { runLedger } from './commands/ledger';
import { runReserves } from './commands/reserves';
import { runRunway } from './commands/runway';
import { runSanitize } from './commands/sanitize';
import { runTransactions } from './commands/transactions';
import { error, log } from './logger';

const COMMANDS: Record<string, (args: string[]) => Promise<void> | void> = {
	accounts: runAccounts,
	'balance-sheet': runBalanceSheet,
	transactions: runTransactions,
	ledger: runLedger,
	cashflow: runCashflow,
	import: runImport,
	sanitize: runSanitize,
	health: runHealth,
	runway: runRunway,
	reserves: runReserves,
	categories: runCategories,
};

const HELP = `
fin - Personal finance CLI

Usage: fin <command> [options]

Global Options:
  --db=PATH     Override database path
  --format=FMT  Output format: table (default), json, tsv

Commands:

  accounts [--group=GROUP]
    List accounts with current balances.
    --group: Filter by group (personal, business, joint)

  balance-sheet [--as-of=DATE]
    Display balance sheet from double-entry ledger.
    --as-of: Balance as of date (YYYY-MM-DD)

  transactions [--account=ID] [--group=GROUP] [--from=DATE] [--to=DATE] [--limit=N]
    Query transactions with filters.
    --account: Filter by chart account ID (e.g., Assets:Checking, Expenses:Food)
    --group: Filter by group (personal, business, joint)
    --from/--to: Date range (YYYY-MM-DD)
    --limit: Max results (default: 50)

  ledger [--account=ID] [--from=DATE] [--to=DATE] [--limit=N]
    Query journal entries from double-entry ledger.
    --account: Filter by account (e.g., Assets:Checking, Expenses:Food)
    --from/--to: Date range (YYYY-MM-DD)
    --limit: Max entries (default: 50)

  cashflow --group=GROUP [--months=N] [--from=DATE]
    Monthly cashflow summary.
    --group: Group ID (personal, business, joint) [required]
    --months: Number of months (default: 12)
    --from: Start month (YYYY-MM)

  import [--inbox=PATH]
    Run import pipeline.
    --inbox: Custom inbox directory (default: imports/inbox)

  sanitize <discover|migrate> [options]
    Discover and migrate journal entry descriptions.
    Subcommands:
      discover [--unmapped] [--min=N] [--account=ID]
        --unmapped: Show only unmapped descriptions
        --min: Minimum occurrences (default: 2)
        --account: Filter by chart account ID
      migrate [--dry-run] [--verbose]
        --dry-run: Preview changes without applying
        --verbose: Show detailed changes

  health --group=GROUP [--from=DATE] [--to=DATE]
    Financial health metrics (balance - reserves).
    --group: Group ID (personal, business, joint) [required]
    --from/--to: Date range (YYYY-MM-DD)

  runway --group=GROUP [--from=DATE] [--to=DATE]
    Months of cash remaining.
    --group: Group ID (personal, business, joint) [required]
    --from/--to: Date range (YYYY-MM-DD)

  reserves --group=GROUP [--from=DATE] [--to=DATE]
    Reserve breakdown (tax + expense reserves).
    --group: Group ID (personal, business, joint) [required]
    --from/--to: Date range (YYYY-MM-DD)

  categories <breakdown|median> --group=GROUP [--months=N] [--limit=N]
    Spending by category.
    Subcommands:
      breakdown: Total spending by category (default)
      median: Monthly median spending by category
    --group: Group ID (personal, business, joint) [required]
    --months: Number of months (default: 3 for breakdown, 6 for median)
    --limit: Max categories (default: 10)
`.trim();

async function main() {
	initConfig();
	const args = Bun.argv.slice(2);

	// Global help
	if (args.includes('--help') || args.includes('-h') || args.includes('help') || args.length === 0) {
		log(HELP);
		process.exit(0);
	}

	const commandIndex = args.findIndex((arg) => !arg.startsWith('-') && arg !== '--');
	const command = commandIndex === -1 ? undefined : args[commandIndex];

	if (!command) {
		log(HELP);
		process.exit(0);
	}

	const handler = COMMANDS[command];
	if (!handler) {
		error(`Unknown command: ${command}\n`);
		log(HELP);
		process.exit(1);
	}

	const globalArgs = commandIndex > 0 ? args.slice(0, commandIndex).filter((arg) => arg !== '--') : [];
	const commandArgs = args.slice(commandIndex + 1);
	await handler([...globalArgs, ...commandArgs]);
}

main().catch((err) => {
	error(`Error: ${err instanceof Error ? err.message : err}`);
	process.exit(1);
});
