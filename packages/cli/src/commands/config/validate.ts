/**
 * `config validate` -- Validate config file.
 *
 * Returns structured validation results with field-specific errors.
 * Runs independently of initConfig() -- parses TOML and validates against schema directly.
 */

import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { FinConfigSchema, findMonorepoRoot, getConfigPath } from '@fin/core/config';
import { emitRaw, fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { defineToolCommand } from '../../tool';

function resolveConfigPath(): string {
	const existing = getConfigPath();
	if (existing) return existing;

	const root = findMonorepoRoot(process.cwd());
	if (root) return join(root, 'data', 'fin.config.toml');
	return join(process.cwd(), 'data', 'fin.config.toml');
}

export const configValidateCommand = defineToolCommand(
	{
		name: 'config.validate',
		command: 'fin config validate',
		category: 'config',
		outputFields: ['valid', 'errors'],
		idempotent: true,
		rateLimit: null,
		example: 'fin config validate --json',
	},
	{
		meta: {
			name: 'validate',
			description: 'Validate config file',
		},
		args: {
			json: {
				type: 'boolean' as const,
				description: 'Output as JSON envelope',
				default: false,
			},
		},
		run({ args: _args }) {
			const start = performance.now();
			const jsonMode = isJsonMode();
			const configPath = resolveConfigPath();

			if (!existsSync(configPath)) {
				if (jsonMode) {
					fail('config.validate', 'NO_CONFIG', `Config file not found: ${configPath}`, 'cp fin.config.template.toml data/fin.config.toml', start);
				}
				process.stderr.write(`Config not found: ${configPath}\n`);
				process.exit(1);
			}

			try {
				const content = readFileSync(configPath, 'utf-8');
				const raw = Bun.TOML.parse(content);
				const result = FinConfigSchema.safeParse(raw);

				if (result.success) {
					const data = { valid: true as const, errors: [] as { path: string; message: string }[] };
					if (jsonMode) {
						ok('config.validate', data, start);
					}
					process.stderr.write(`Config valid: ${configPath}\n`);
					return;
				}

				const errors = result.error.issues.map((issue) => ({
					path: issue.path.join('.'),
					message: issue.message,
				}));

				if (jsonMode) {
					const data = { valid: false as const, errors };
					const elapsed = Math.round(performance.now() - start);
					const envelope = { ok: true, data, meta: { tool: 'config.validate', elapsed } };
					emitRaw(JSON.stringify(envelope), 1);
				}

				process.stderr.write(`Config invalid: ${configPath}\n`);
				for (const err of errors) {
					process.stderr.write(`  ${err.path}: ${err.message}\n`);
				}
				process.exit(1);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				if (jsonMode) {
					fail('config.validate', 'INVALID_CONFIG', `Failed to parse config: ${message}`, 'Check TOML syntax', start);
				}
				process.stderr.write(`Parse error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
