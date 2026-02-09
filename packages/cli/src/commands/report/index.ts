/**
 * `report` command group -- Financial analytics and reports.
 *
 * All subcommands use defineToolCommand() with --json envelope support.
 */

import { defineCommand } from 'citty';
import { reportAuditCommand } from './audit';
import { reportCashflowCommand } from './cashflow';
import { reportCategoriesCommand } from './categories';
import { reportHealthCommand } from './health';
import { reportReservesCommand } from './reserves';
import { reportRunwayCommand } from './runway';
import { reportSummaryCommand } from './summary';

export const reportCommand = defineCommand({
	meta: {
		name: 'report',
		description: 'Financial analytics and reports',
	},
	subCommands: {
		cashflow: reportCashflowCommand,
		health: reportHealthCommand,
		runway: reportRunwayCommand,
		reserves: reportReservesCommand,
		categories: reportCategoriesCommand,
		audit: reportAuditCommand,
		summary: reportSummaryCommand,
	},
});
