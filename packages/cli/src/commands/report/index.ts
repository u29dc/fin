/**
 * `report` command group -- Financial analytics and reports.
 *
 * Migrated subcommands use defineToolCommand() with --json envelope support.
 * Legacy subcommands are imported from report.legacy.ts until they are migrated.
 */

import { defineCommand } from 'citty';
import { summary } from '../report.legacy';
import { reportAuditCommand } from './audit';
import { reportCashflowCommand } from './cashflow';
import { reportCategoriesCommand } from './categories';
import { reportHealthCommand } from './health';
import { reportReservesCommand } from './reserves';
import { reportRunwayCommand } from './runway';

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
		summary,
	},
});
