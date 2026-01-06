import { existsSync, readFileSync } from 'node:fs';
import { dirname, isAbsolute, join } from 'node:path';
import { parse as parseTOML } from 'smol-toml';
import { type FinConfig, FinConfigSchema } from './schema';

let configSingleton: FinConfig | null = null;
let configPath: string | null = null;

/**
 * Walk up directory tree to find monorepo root (where fin.config.template.toml exists).
 */
export function findMonorepoRoot(startDir: string): string | null {
	let dir = startDir;
	while (dir !== dirname(dir)) {
		if (existsSync(join(dir, 'fin.config.template.toml'))) {
			return dir;
		}
		dir = dirname(dir);
	}
	return null;
}

function findConfigPath(startPath?: string): string {
	// If explicit path provided, use it
	if (startPath) {
		return isAbsolute(startPath) ? startPath : join(process.cwd(), startPath);
	}

	// Check for FIN_CONFIG_PATH environment variable
	const envPath = process.env['FIN_CONFIG_PATH'];
	if (envPath) {
		return isAbsolute(envPath) ? envPath : join(process.cwd(), envPath);
	}

	// Check for FIN_HOME environment variable (project root directory)
	const homeDir = process.env['FIN_HOME'];
	if (homeDir) {
		const homePath = isAbsolute(homeDir) ? homeDir : join(process.cwd(), homeDir);
		return join(homePath, 'data', 'fin.config.toml');
	}

	// Walk up to find monorepo root (handles running from packages/web/ etc.)
	const root = findMonorepoRoot(process.cwd());
	if (root) {
		return join(root, 'data', 'fin.config.toml');
	}

	// Fallback to cwd-relative path
	return join(process.cwd(), 'data', 'fin.config.toml');
}

export function loadConfig(path?: string): FinConfig {
	const resolvedPath = findConfigPath(path);

	if (!existsSync(resolvedPath)) {
		throw new Error(`Config file not found: ${resolvedPath}\nCopy fin.config.template.toml to data/fin.config.toml and customize it.`);
	}

	// Read and parse TOML (compatible with Vite SSR)
	const content = readFileSync(resolvedPath, 'utf-8');
	const data = parseTOML(content);

	// Validate with Zod
	const result = FinConfigSchema.safeParse(data);

	if (!result.success) {
		const errors = result.error.issues.map((e) => `  ${e.path.join('.')}: ${e.message}`).join('\n');
		throw new Error(`Invalid config file at ${resolvedPath}:\n${errors}`);
	}

	return result.data;
}

export function initConfig(path?: string): void {
	configPath = findConfigPath(path);
	configSingleton = loadConfig(configPath);
}

export function getConfig(): FinConfig {
	if (!configSingleton) {
		throw new Error('Config not initialized. Call initConfig() first.');
	}
	return configSingleton;
}

export function isConfigInitialized(): boolean {
	return configSingleton !== null;
}

export function getConfigPath(): string | null {
	return configPath;
}

export function getConfigDir(): string | null {
	return configPath ? dirname(configPath) : null;
}

export function resetConfig(): void {
	configSingleton = null;
	configPath = null;
}
