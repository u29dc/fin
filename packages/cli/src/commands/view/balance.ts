/**
 * `view balance` -- Display balance sheet.
 *
 * Returns assets, liabilities, equity, income, expenses, net worth, and net income.
 * Supports --as-of date filter.
 */

import { getBalanceSheet } from '@fin/core';
import { getReadonlyDb } from '../../db';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { formatAmount } from '../../format';
import { type Column, parseFormat, renderOutput } from '../../output';
import { defineToolCommand } from '../../tool';

type BalanceRow = {
	category: string;
	amount: number;
};

const BALANCE_COLUMNS: Column<BalanceRow>[] = [
	{ key: 'category', label: 'Category', minWidth: 20 },
	{ key: 'amount', label: 'Amount', align: 'right', minWidth: 15, format: (v) => formatAmount(v as number) },
];

export const viewBalanceCommand = defineToolCommand(
	{
		name: 'view.balance',
		command: 'fin view balance',
		category: 'view',
		outputSchema: {
			assets: { type: 'number', description: 'Total assets (minor units)' },
			liabilities: { type: 'number', description: 'Total liabilities (minor units)' },
			equity: { type: 'number', description: 'Total equity (minor units)' },
			income: { type: 'number', description: 'Total income (minor units)' },
			expenses: { type: 'number', description: 'Total expenses (minor units)' },
			netWorth: { type: 'number', description: 'Net worth: assets - liabilities (minor units)' },
			netIncome: { type: 'number', description: 'Net income: income - expenses (minor units)' },
		},
		idempotent: true,
		rateLimit: null,
		example: 'fin view balance --as-of=2025-12-31 --json',
	},
	{
		meta: {
			name: 'balance',
			description: 'Display balance sheet',
		},
		args: {
			'as-of': { type: 'string' as const, description: 'Balance as of date (YYYY-MM-DD)' },
			json: { type: 'boolean' as const, description: 'Output as JSON envelope', default: false },
			db: { type: 'string' as const, description: 'Database path' },
			format: { type: 'string' as const, description: 'Output format: table, json, tsv', default: 'table' },
		},
		run({ args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();

			try {
				const db = getReadonlyDb(args.db ? { options: new Map([['db', args.db]]) } : undefined);
				const bs = getBalanceSheet(db, args['as-of']);

				if (jsonMode) {
					ok('view.balance', bs, start);
				}

				const rows: BalanceRow[] = [
					{ category: 'Assets', amount: bs.assets },
					{ category: 'Liabilities', amount: bs.liabilities },
					{ category: 'Net Worth', amount: bs.netWorth },
					{ category: '', amount: 0 },
					{ category: 'Income', amount: bs.income },
					{ category: 'Expenses', amount: bs.expenses },
					{ category: 'Net Income', amount: bs.netIncome },
				];

				const format = parseFormat(args.format);
				const outputRows = format === 'json' ? rows.filter((r) => r.category !== '') : rows;
				renderOutput(outputRows, BALANCE_COLUMNS, format);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('view.balance', 'DB_ERROR', `Failed to query balance sheet: ${message}`, 'Check database at data/fin.db', start);
				}
				process.stderr.write(`Error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
