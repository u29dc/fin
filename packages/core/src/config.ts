import { getFinancialConfig } from './config/index';
import type { ScenarioToggles } from './queries/metrics';

// Re-export new config accessors
export { getAccountIdsByGroup, getFirstAccountIdByGroup, getLiquidAccountIds } from './config/index';
export type { ScenarioToggles } from './queries/metrics';

export type FinanceConfig = {
	corpTaxRate: number;
	vatRate: number;
	personalDividendTax: {
		allowanceMinor: number;
		basicRate: number;
		higherRate: number;
	};
	personalIncomeTaxRate: number;
	jointShareYou: number;
	scenario: {
		lookbackMonths: number;
		salaryDividendSplitMinor: number;
		dividendsMonthlyMinor: number;
		salaryMonthlyMinor: number;
		jointExpensesMonthlyMinor: number;
	};
	scenarioToggles: ScenarioToggles;
	fixedMonthlyPersonalOutflowMinor: number | null;
	expenseReserveMonths: number;
	trailingExpenseWindowMonths: number;
	investmentProjectionAnnualReturns: {
		low: number;
		mid: number;
		high: number;
	};
	runwayThresholdMinor?: number | undefined;
	runwayWarningMinor?: number | undefined;
};

/**
 * Get finance config from TOML configuration.
 * Adapts TOML snake_case keys to existing camelCase FinanceConfig type.
 */
export function getFinanceConfig(): FinanceConfig {
	const cfg = getFinancialConfig();
	const toggles = cfg.scenario.toggles;
	return {
		corpTaxRate: cfg.corp_tax_rate,
		vatRate: cfg.vat_rate,
		personalDividendTax: {
			allowanceMinor: cfg.personal_dividend_tax.allowance_minor,
			basicRate: cfg.personal_dividend_tax.basic_rate,
			higherRate: cfg.personal_dividend_tax.higher_rate,
		},
		personalIncomeTaxRate: cfg.personal_income_tax_rate,
		jointShareYou: cfg.joint_share_you,
		scenario: {
			lookbackMonths: cfg.scenario.lookback_months,
			salaryDividendSplitMinor: cfg.scenario.salary_dividend_split_minor,
			dividendsMonthlyMinor: cfg.scenario.dividends_monthly_minor,
			salaryMonthlyMinor: cfg.scenario.salary_monthly_minor,
			jointExpensesMonthlyMinor: cfg.scenario.joint_expenses_monthly_minor,
		},
		scenarioToggles: {
			includeDividends: toggles?.include_dividends ?? true,
			includeSalary: toggles?.include_salary ?? true,
			includeJointExpenses: toggles?.include_joint_expenses ?? true,
		},
		fixedMonthlyPersonalOutflowMinor: cfg.fixed_monthly_personal_outflow_minor,
		expenseReserveMonths: cfg.expense_reserve_months,
		trailingExpenseWindowMonths: cfg.trailing_expense_window_months,
		investmentProjectionAnnualReturns: cfg.investment_projection_annual_returns,
		runwayThresholdMinor: cfg.runway_threshold_minor,
		runwayWarningMinor: cfg.runway_warning_minor,
	};
}
