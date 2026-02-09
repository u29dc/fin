/**
 * report.legacy -- Legacy report subcommands not yet migrated to defineToolCommand().
 *
 * These are imported by report/index.ts alongside migrated commands.
 * Each command will be moved to its own file in report/ as it is migrated.
 */

import { summary } from './summary';

// Re-export summary for use by report/index.ts
export { summary };
