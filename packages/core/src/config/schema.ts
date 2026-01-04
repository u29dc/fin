import { z } from 'zod';

// Group metadata schema for configurable account groups
const GroupMetadataSchema = z.object({
	id: z.string(), // e.g., "personal", "business", "savings-pool"
	label: z.string(), // Display label for UI
	icon: z.enum(['user', 'briefcase', 'heart', 'building', 'wallet', 'piggy-bank']).default('wallet'),
	tax_type: z.enum(['corp', 'income', 'none']).default('none'),
	expense_reserve_months: z.number().default(3),
});

// Account schema
const AccountSchema = z.object({
	id: z.string(), // e.g., "Assets:Personal:Monzo"
	group: z.string(), // e.g., "personal", "joint", "business" - any string, validated against config
	type: z.enum(['asset', 'liability', 'equity', 'income', 'expense']),
	provider: z.string(), // e.g., "monzo", "wise", "vanguard"
	label: z.string().optional(), // Display label for UI
	subtype: z.enum(['checking', 'savings', 'investment']).optional(), // For asset allocation display
	inbox_folder: z.string().optional(), // Folder name in imports/inbox/ for this account
});

// Bank preset schema for CSV column mappings
const BankColumnsSchema = z.object({
	date: z.string(),
	time: z.string().optional(),
	description: z.string(),
	amount: z.string(),
	balance: z.string().optional(),
	transaction_id: z.string().optional(),
	name: z.string().optional(),
	category: z.string().optional(),
});

const BankPresetSchema = z.object({
	name: z.string(), // e.g., "monzo", "wise", "vanguard"
	columns: BankColumnsSchema,
});

// Financial config schema
const PersonalDividendTaxSchema = z.object({
	allowance_minor: z.number(),
	basic_rate: z.number(),
	higher_rate: z.number(),
});

const ScenarioTogglesSchema = z.object({
	include_dividends: z.boolean().optional().default(true),
	include_salary: z.boolean().optional().default(true),
	include_joint_expenses: z.boolean().optional().default(true),
});

const ScenarioSchema = z.object({
	lookback_months: z.number(),
	salary_dividend_split_minor: z.number(),
	dividends_monthly_minor: z.number(),
	salary_monthly_minor: z.number(),
	joint_expenses_monthly_minor: z.number(),
	toggles: ScenarioTogglesSchema.optional(),
});

const InvestmentProjectionSchema = z.object({
	low: z.number(),
	mid: z.number(),
	high: z.number(),
});

const FinancialSchema = z.object({
	corp_tax_rate: z.number(),
	vat_rate: z.number(),
	personal_dividend_tax: PersonalDividendTaxSchema,
	personal_income_tax_rate: z.number(),
	joint_share_you: z.number(),
	expense_reserve_months: z.number(),
	trailing_expense_window_months: z.number(),
	scenario: ScenarioSchema,
	fixed_monthly_personal_outflow_minor: z.number().nullable(),
	investment_projection_annual_returns: InvestmentProjectionSchema,
	runway_threshold_minor: z.number().optional(), // Minimum runway balance (e.g., 4000000 = 40K GBP)
	runway_warning_minor: z.number().optional(), // Warning line for runway chart (e.g., 5000000 = 50K GBP)
});

// Sanitization config schema
const SanitizationSchema = z.object({
	rules: z.string().optional(), // Path to rules file (e.g., "data/fin.rules.ts")
});

// Full config schema
export const FinConfigSchema = z.object({
	financial: FinancialSchema,
	accounts: z.array(AccountSchema),
	banks: z.array(BankPresetSchema),
	sanitization: SanitizationSchema.optional(),
	groups: z.array(GroupMetadataSchema).optional(), // Optional: explicit group definitions
});

export type FinConfig = z.infer<typeof FinConfigSchema>;
export type Account = z.infer<typeof AccountSchema>;
export type AccountSubtype = 'checking' | 'savings' | 'investment';
export type BankPreset = z.infer<typeof BankPresetSchema>;
export type BankColumns = z.infer<typeof BankColumnsSchema>;
export type GroupMetadata = z.infer<typeof GroupMetadataSchema>;
export type GroupId = string; // Dynamic: any group ID defined in config
