/**
 * CLI logging utility
 *
 * Centralizes console output to avoid biome noConsole warnings throughout codebase.
 * Simple wrapper - no timestamps or domains needed for this CLI tool.
 */

/** Quiet mode suppresses all log output (useful for tests) */
let quietMode = false;

/**
 * Enable/disable quiet mode
 */
export function setQuietMode(quiet: boolean): void {
	quietMode = quiet;
}

/**
 * Log to stdout
 */
export function log(message: string): void {
	if (quietMode) return;
	// biome-ignore lint/suspicious/noConsole: intentional CLI output
	console.log(message);
}

/**
 * Log to stderr
 */
export function error(message: string): void {
	if (quietMode) return;
	// biome-ignore lint/suspicious/noConsole: intentional CLI output
	console.error(message);
}

/**
 * Log JSON to stdout (for --format=json)
 */
export function json(data: unknown): void {
	if (quietMode) return;
	// biome-ignore lint/suspicious/noConsole: intentional CLI output
	console.log(JSON.stringify(data, null, 2));
}
