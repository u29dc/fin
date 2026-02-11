import { homedir } from 'node:os';
import { join } from 'node:path';

/**
 * Resolve fin tool home directory.
 *
 * Precedence: FIN_HOME -> TOOLS_HOME/fin -> $HOME/.tools/fin
 */
export function resolveFinHome(): string {
	const finHome = process.env['FIN_HOME'];
	if (finHome) return finHome;
	const toolsHome = process.env['TOOLS_HOME'] || join(homedir(), '.tools');
	return join(toolsHome, 'fin');
}

/** All derived fin paths from tool home */
export function resolveFinPaths() {
	const home = resolveFinHome();
	return {
		home,
		dataDir: join(home, 'data'),
		configFile: join(home, 'data', 'fin.config.toml'),
		dbFile: join(home, 'data', 'fin.db'),
		rulesFile: join(home, 'data', 'fin.rules.ts'),
		backupsDir: join(home, 'data', 'backups'),
		inboxDir: join(home, 'imports', 'inbox'),
		archiveDir: join(home, 'imports', 'archive'),
	};
}
