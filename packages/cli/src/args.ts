/**
 * Lightweight argument parsing utilities.
 * No external dependencies - uses native Bun.argv parsing.
 */

import { type GroupId, isGroupId } from 'core';

export type ParsedArgs = {
	/** Boolean flags like --verbose, --json */
	flags: Set<string>;
	/** Key-value options like --limit=50, --group=personal */
	options: Map<string, string>;
	/** Positional arguments (subcommands, etc.) */
	positional: string[];
};

type LongOptionResult = { key: string; value?: string; skip: number };

function parseLongOption(arg: string, args: string[], index: number): LongOptionResult | null {
	if (!arg.startsWith('--')) return null;

	const withoutPrefix = arg.slice(2);
	const eqIndex = withoutPrefix.indexOf('=');

	if (eqIndex !== -1) {
		return {
			key: withoutPrefix.slice(0, eqIndex),
			value: withoutPrefix.slice(eqIndex + 1),
			skip: 0,
		};
	}

	const next = args[index + 1];
	if (next && !next.startsWith('-')) {
		return { key: withoutPrefix, value: next, skip: 1 };
	}

	return { key: withoutPrefix, skip: 0 };
}

/**
 * Parse command-line arguments into a structured format.
 * Supports:
 * - Boolean flags: --verbose, --json
 * - Key-value options: --limit=50, --group=personal
 * - Positional arguments: discover, migrate
 */
export function parseArgs(args: string[]): ParsedArgs {
	const flags = new Set<string>();
	const options = new Map<string, string>();
	const positional: string[] = [];

	let passthrough = false;

	for (let i = 0; i < args.length; i += 1) {
		const arg = args[i];
		if (arg === undefined) continue;

		if (passthrough) {
			positional.push(arg);
			continue;
		}

		if (arg === '--') {
			passthrough = true;
			continue;
		}

		const optionResult = parseLongOption(arg, args, i);
		if (optionResult) {
			if (optionResult.value !== undefined) {
				options.set(optionResult.key, optionResult.value);
			} else {
				flags.add(optionResult.key);
			}
			i += optionResult.skip;
			continue;
		}

		if (arg.startsWith('-') && arg !== '-') {
			flags.add(arg.slice(1));
			continue;
		}

		positional.push(arg);
	}

	return { flags, options, positional };
}

/**
 * Check if a boolean flag is present.
 */
export function hasFlag(parsed: ParsedArgs, name: string): boolean {
	return parsed.flags.has(name);
}

/**
 * Get an option value, or undefined if not set.
 */
export function getOption(parsed: ParsedArgs, name: string): string | undefined {
	return parsed.options.get(name);
}

/**
 * Get an option value, or a default if not set.
 */
export function getOptionOrDefault(parsed: ParsedArgs, name: string, defaultValue: string): string {
	return parsed.options.get(name) ?? defaultValue;
}

/**
 * Get a required option value, or throw an error with usage hint.
 */
export function requireOption(parsed: ParsedArgs, name: string, commandName: string): string {
	const value = parsed.options.get(name);
	if (value === undefined) {
		throw new Error(`Missing required option: --${name}\nRun: bun run cli ${commandName} --help`);
	}
	return value;
}

/**
 * Get an option as a number, or undefined if not set.
 * Throws if the value is not a valid number.
 */
export function getOptionAsNumber(parsed: ParsedArgs, name: string): number | undefined {
	const value = parsed.options.get(name);
	if (value === undefined) return undefined;

	const num = Number(value);
	if (Number.isNaN(num)) {
		throw new Error(`Invalid number for --${name}: ${value}`);
	}
	return num;
}

/**
 * Get an option as a number with a default.
 */
export function getOptionAsNumberOrDefault(parsed: ParsedArgs, name: string, defaultValue: number): number {
	return getOptionAsNumber(parsed, name) ?? defaultValue;
}

/**
 * Validate and narrow a group ID option. Exits with error if invalid.
 */
export function validateGroupId(value: string | undefined, commandName: string): asserts value is GroupId | undefined {
	if (value && !isGroupId(value)) {
		console.error(`Invalid group: ${value}. Use: personal, business, joint`);
		console.error(`Run: bun run cli ${commandName} --help`);
		process.exit(1);
	}
}
