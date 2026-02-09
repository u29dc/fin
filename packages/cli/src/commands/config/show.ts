/**
 * `config show` -- Show parsed configuration.
 *
 * Returns the full parsed config (groups, accounts, financial parameters).
 * Useful for an agent to understand what groups and accounts exist.
 */

import { type GroupMetadata, getAccountsByGroup, getAllGroupMetadata, getConfigPath, getFinancialConfig, getGroupIds } from '@fin/core/config';
import { fail, isJsonMode, ok, rethrowCapture } from '../../envelope';
import { defineToolCommand } from '../../tool';

type AccountSummary = { id: string; provider: string; label?: string; subtype?: string };

function buildAccountMap(groupIds: string[]): Record<string, AccountSummary[]> {
	const accounts: Record<string, AccountSummary[]> = {};
	for (const gid of groupIds) {
		accounts[gid] = getAccountsByGroup(gid).map((a) => ({
			id: a.id,
			provider: a.provider,
			...(a.label ? { label: a.label } : {}),
			...(a.subtype ? { subtype: a.subtype } : {}),
		}));
	}
	return accounts;
}

function renderText(configPath: string | null, groups: GroupMetadata[], accounts: Record<string, AccountSummary[]>, financial: ReturnType<typeof getFinancialConfig>): void {
	const w = (s: string) => process.stderr.write(`${s}\n`);
	w(`Config: ${configPath}`);
	w('');
	w('Groups:');
	for (const g of groups) {
		w(`  ${g.id} (${g.label}) -- tax: ${g.taxType}, reserve: ${g.expenseReserveMonths}mo`);
		const accts = accounts[g.id];
		if (accts) {
			for (const a of accts) {
				w(`    ${a.id} [${a.provider}]${a.label ? ` "${a.label}"` : ''}`);
			}
		}
	}
	w('');
	w('Financial:');
	w(`  Corp tax rate: ${(financial.corp_tax_rate * 100).toFixed(0)}%`);
	w(`  VAT rate: ${(financial.vat_rate * 100).toFixed(0)}%`);
	w(`  Personal income tax: ${(financial.personal_income_tax_rate * 100).toFixed(0)}%`);
	w(`  Joint share: ${(financial.joint_share_you * 100).toFixed(0)}%`);
	w(`  Expense reserve months: ${financial.expense_reserve_months}`);
}

export const configShowCommand = defineToolCommand(
	{
		name: 'config.show',
		command: 'fin config show',
		category: 'config',
		outputFields: ['groups', 'accounts', 'financial', 'configPath'],
		idempotent: true,
		rateLimit: null,
		example: 'fin config show --json',
	},
	{
		meta: {
			name: 'show',
			description: 'Show parsed configuration',
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

			try {
				const configPath = getConfigPath();
				const groupIds = getGroupIds();
				const groups = getAllGroupMetadata();
				const financial = getFinancialConfig();
				const accounts = buildAccountMap(groupIds);

				const data = { groups, accounts, financial, configPath: configPath ?? 'unknown' };

				if (jsonMode) {
					ok('config.show', data, start);
				}

				renderText(configPath, groups, accounts, financial);
			} catch (error) {
				rethrowCapture(error);
				const message = error instanceof Error ? error.message : String(error);
				const isNotFound = message.includes('not found') || message.includes('ENOENT') || message.includes('not initialized');

				if (isNotFound) {
					if (jsonMode) {
						fail('config.show', 'NO_CONFIG', `Config not available: ${message}`, 'cp fin.config.template.toml data/fin.config.toml', start);
					}
					process.stderr.write(`Error: ${message}\n`);
					process.exit(1);
				}

				if (jsonMode) {
					fail('config.show', 'INVALID_CONFIG', `Config error: ${message}`, 'Check config file against template', start);
				}
				process.stderr.write(`Config error: ${message}\n`);
				process.exit(1);
			}
		},
	},
);
