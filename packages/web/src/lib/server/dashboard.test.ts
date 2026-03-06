import { describe, expect, test } from "bun:test";

import { loadDashboardPageData } from "$lib/server/dashboard";
import type {
	ConfigShowData,
	DashboardAllocationData,
	DashboardContributionData,
	DashboardFlowData,
	DashboardHierarchyData,
	DashboardKpisData,
	FinApiClient,
	HealthReport,
	RunwayReport,
	SummaryReport,
	ViewAccountsData,
	CashflowReport,
	DashboardBalanceData,
} from "$lib/server/api";

function createMockClient(overrides: Partial<FinApiClient>): FinApiClient {
	return {
		transport: { kind: "tcp", origin: "http://127.0.0.1:7414" },
		get: async () => {
			throw new Error("not implemented");
		},
		configShow: async () => {
			throw new Error("configShow not implemented");
		},
		health: async () => ({
			status: "ready",
			checks: [],
			summary: { ok: 1, blocking: 0, degraded: 0 },
		}),
		viewAccounts: async () => ({ accounts: [], total: 0 }),
		reportSummary: async () => ({
			generated_at: "now",
			period_months: 12,
			groups: {},
			consolidated: {
				net_worth_minor: 0,
				balance_sheet: {
					assets: 0,
					liabilities: 0,
					equity: 0,
					income: 0,
					expenses: 0,
					net_worth: 0,
					net_income: 0,
				},
			},
		}),
		reportCashflow: async () => ({ series: [], totals: { income_minor: 0, expense_minor: 0, net_minor: 0 } }),
		reportRunway: async () => ({ series: [], latest: null, groups: [] }),
		dashboardKpis: async (group: string) => ({
			groupId: group,
			groupLabel: group,
			months: 12,
			kpis: {
				current_month: "2026-03",
				current_partial_month: null,
				last_full_month: null,
				previous_full_month: null,
				trailing_average_net_minor: null,
				median_spend_minor: null,
				short_term_trend: null,
				anomaly_count_last_12_months: 0,
				recent_anomaly_months: [],
			},
		}),
		dashboardAllocation: async (group: string) => ({
			reportingMonth: "2026-03",
			snapshot: {
				group_id: group,
				group_label: group,
				net_total_minor: 0,
				positive_total_minor: 0,
				account_segments: [],
				dashboard: {
					basis: "reserve_composition",
					balance_basis_minor: 0,
					display_total_minor: 0,
					available_minor: 0,
					expense_reserve_minor: 0,
					expense_reserve_display_minor: 0,
					tax_reserve_minor: 0,
					emergency_fund_minor: 0,
					savings_minor: 0,
					investment_minor: 0,
					shortfall_minor: 0,
					under_reserved: false,
					segments: [],
				},
			},
		}),
		dashboardFlow: async (group: string) => ({
			groupId: group,
			months: 6,
			mode: "monthly_average",
			graph: { total_minor: 0, nodes: [], edges: [] },
		}),
		dashboardHierarchy: async (group: string) => ({
			groupId: group,
			months: 6,
			mode: "monthly_average",
			totalMinor: 0,
			nodes: [],
		}),
		dashboardBalances: async () => ({
			scopeKind: "account",
			scopeId: "",
			scopeLabel: "",
			series: [],
		}),
		dashboardContributions: async () => ({ accountId: "", accountLabel: "", series: [] }),
		dashboardProjection: async () => ({
			groups: [],
			report: {
				scope_kind: "group",
				scope_id: "",
				liquid_balance_minor: 0,
				current_burn_minor: 0,
				minimum_burn_minor: 0,
				median_monthly_expense_minor: 0,
				thresholds: { warning_minor: null, threshold_minor: null },
				assumptions: {
					as_of_date: "2026-03-01",
					projection_months: 12,
					trailing_outflow_window_months: 12,
					burn_rate_method: "median",
					minimum_burn_ratio: 0.5,
					full_months_only: true,
					include_as_of_month_in_history: false,
				},
				scenarios: [],
			},
		}),
		...overrides,
	};
}

describe("loadDashboardPageData", () => {
	test("maps fin-api payloads into dashboard page data", async () => {
		const configShow: ConfigShowData = {
			groups: [
				{ id: "business", label: "Business", icon: "briefcase", taxType: "corp", expenseReserveMonths: 12 },
				{ id: "personal", label: "Personal", icon: "user", taxType: "income", expenseReserveMonths: 6 },
			],
			accounts: {
				personal: [
					{ id: "Assets:Personal:Monzo", label: "Personal Monzo", provider: "monzo" },
					{
						id: "Assets:Personal:Investments",
						label: "Personal Investments",
						provider: "vanguard",
						subtype: "investment",
					},
				],
				business: [{ id: "Assets:Business:Monzo", label: "Business Monzo", provider: "monzo" }],
			},
			financial: {
				investment_projection_annual_returns: { low: 0.04, mid: 0.06, high: 0.08 },
			},
			configPath: "/tmp/fin.config.toml",
		};
		const health: HealthReport = {
			status: "ready",
			checks: [],
			summary: { ok: 2, blocking: 0, degraded: 0 },
		};
		const accounts: ViewAccountsData = {
			accounts: [
				{
					id: "Assets:Personal:Monzo",
					name: "Personal Monzo",
					account_type: "asset",
					balance_minor: 120_000,
					updated_at: "2026-03-01",
				},
				{
					id: "Assets:Personal:Investments",
					name: "Personal Investments",
					account_type: "asset",
					balance_minor: 500_000,
					updated_at: "2026-03-01",
				},
				{
					id: "Assets:Business:Monzo",
					name: "Business Monzo",
					account_type: "asset",
					balance_minor: 900_000,
					updated_at: "2026-03-01",
				},
			],
			total: 3,
		};
		const summary: SummaryReport = {
			generated_at: "now",
			period_months: 12,
			groups: {
				personal: {
					label: "Personal",
					net_worth_minor: 620_000,
					latest_runway_months: 8.2,
					latest_health_minor: 420_000,
					latest_available_minor: 180_000,
					last_full_month_net_minor: 45_000,
					trailing_average_net_minor: 12_000,
					median_spend_minor: 75_000,
					short_term_trend: "positive",
					anomaly_count_last_12_months: 2,
				},
				business: {
					label: "Business",
					net_worth_minor: 900_000,
					latest_runway_months: 16.5,
					latest_health_minor: 1_200_000,
					latest_available_minor: 700_000,
					last_full_month_net_minor: 80_000,
					trailing_average_net_minor: 40_000,
					median_spend_minor: 210_000,
					short_term_trend: "flat",
					anomaly_count_last_12_months: 1,
				},
			},
			consolidated: {
				net_worth_minor: 1_520_000,
				balance_sheet: {
					assets: 1_520_000,
					liabilities: 0,
					equity: 0,
					income: 0,
					expenses: 0,
					net_worth: 1_520_000,
					net_income: 0,
				},
			},
		};
		const kpisByGroup: Record<string, DashboardKpisData> = {
			personal: {
				groupId: "personal",
				groupLabel: "Personal",
				months: 12,
				kpis: {
					current_month: "2026-03",
					current_partial_month: null,
					last_full_month: {
						month: "2026-02",
						income_minor: 200_000,
						expense_minor: 155_000,
						net_minor: 45_000,
						savings_rate_pct: 22.5,
						rolling_median_expense_minor: 75_000,
						expense_deviation_ratio: 1.1,
						is_anomaly: false,
					},
					previous_full_month: null,
					trailing_average_net_minor: 12_000,
					median_spend_minor: 75_000,
					short_term_trend: "positive",
					anomaly_count_last_12_months: 2,
					recent_anomaly_months: ["2025-12", "2026-02"],
				},
			},
			business: {
				groupId: "business",
				groupLabel: "Business",
				months: 12,
				kpis: {
					current_month: "2026-03",
					current_partial_month: null,
					last_full_month: null,
					previous_full_month: null,
					trailing_average_net_minor: 40_000,
					median_spend_minor: 210_000,
					short_term_trend: "flat",
					anomaly_count_last_12_months: 1,
					recent_anomaly_months: ["2025-11"],
				},
			},
		};
		const cashflowByGroup: Record<string, CashflowReport> = {
			personal: {
				series: [
					{
						month: "2026-01",
						income_minor: 180_000,
						expense_minor: 160_000,
						net_minor: 20_000,
						savings_rate_pct: 11.1,
						rolling_median_expense_minor: 75_000,
						expense_deviation_ratio: 1.0,
						is_anomaly: false,
					},
					{
						month: "2026-02",
						income_minor: 200_000,
						expense_minor: 155_000,
						net_minor: 45_000,
						savings_rate_pct: 22.5,
						rolling_median_expense_minor: 75_000,
						expense_deviation_ratio: 1.1,
						is_anomaly: false,
					},
				],
				totals: { income_minor: 380_000, expense_minor: 315_000, net_minor: 65_000 },
			},
			business: { series: [], totals: { income_minor: 0, expense_minor: 0, net_minor: 0 } },
		};
		const runwayByGroup: Record<string, RunwayReport> = {
			personal: {
				series: [],
				latest: {
					date: "2026-03-01",
					runway_months: 8.2,
					balance_minor: 620_000,
					burn_rate_minor: 75_000,
					median_expense_minor: 75_000,
				},
				groups: ["personal"],
			},
			business: {
				series: [],
				latest: {
					date: "2026-03-01",
					runway_months: 16.5,
					balance_minor: 900_000,
					burn_rate_minor: -20_000,
					median_expense_minor: 210_000,
				},
				groups: ["business"],
			},
		};
		const allocationByGroup: Record<string, DashboardAllocationData> = {
			personal: {
				reportingMonth: "2026-03",
				snapshot: {
					group_id: "personal",
					group_label: "Personal",
					net_total_minor: 620_000,
					positive_total_minor: 620_000,
					account_segments: [],
					dashboard: {
						basis: "personal_buffer",
						balance_basis_minor: 600_000,
						display_total_minor: 620_000,
						available_minor: 180_000,
						expense_reserve_minor: 320_000,
						expense_reserve_display_minor: 320_000,
						tax_reserve_minor: 0,
						emergency_fund_minor: 0,
						savings_minor: 0,
						investment_minor: 120_000,
						shortfall_minor: 0,
						under_reserved: false,
						segments: [
							{
								bucket: "available_cash",
								label: "Available Cash",
								amount_minor: 180_000,
								share_pct: 29.03,
								account_ids: ["Assets:Personal:Monzo"],
								derived: true,
							},
							{
								bucket: "expense_reserve",
								label: "Expense Reserve",
								amount_minor: 320_000,
								share_pct: 51.61,
								account_ids: ["Assets:Personal:Monzo"],
								derived: true,
							},
							{
								bucket: "investment",
								label: "Investments",
								amount_minor: 120_000,
								share_pct: 19.35,
								account_ids: ["Assets:Personal:Investments"],
								derived: false,
							},
						],
					},
				},
			},
			business: {
				reportingMonth: "2026-03",
				snapshot: {
					group_id: "business",
					group_label: "Business",
					net_total_minor: 900_000,
					positive_total_minor: 900_000,
					account_segments: [],
					dashboard: {
						basis: "reserve_composition",
						balance_basis_minor: 900_000,
						display_total_minor: 900_000,
						available_minor: 700_000,
						expense_reserve_minor: 120_000,
						expense_reserve_display_minor: 120_000,
						tax_reserve_minor: 80_000,
						emergency_fund_minor: 0,
						savings_minor: 0,
						investment_minor: 0,
						shortfall_minor: 0,
						under_reserved: false,
						segments: [],
					},
				},
			},
		};
		const flowByGroup: Record<string, DashboardFlowData> = {
			personal: {
				groupId: "personal",
				months: 6,
				mode: "monthly_average",
				graph: {
					total_minor: 200_000,
					nodes: [
						{ id: "Income:Salary", label: "Salary", kind: "income" },
						{ id: "Assets:Personal:Monzo", label: "Personal Monzo", kind: "asset" },
						{ id: "Expenses:Housing:Rent", label: "Rent", kind: "expense" },
					],
					edges: [
						{
							source_id: "Income:Salary",
							target_id: "Assets:Personal:Monzo",
							amount_minor: 200_000,
							share_of_total_pct: 100,
							share_of_source_pct: 100,
						},
						{
							source_id: "Assets:Personal:Monzo",
							target_id: "Expenses:Housing:Rent",
							amount_minor: 120_000,
							share_of_total_pct: 60,
							share_of_source_pct: 60,
						},
					],
				},
			},
			business: { groupId: "business", months: 6, mode: "monthly_average", graph: { total_minor: 0, nodes: [], edges: [] } },
		};
		const hierarchyByGroup: Record<string, DashboardHierarchyData> = {
			personal: {
				groupId: "personal",
				months: 6,
				mode: "monthly_average",
				totalMinor: 155_000,
				nodes: [
					{
						account_id: "Expenses:Housing",
						name: "Housing",
						kind: "expense",
						total_minor: 120_000,
						share_of_parent_pct: 100,
						share_of_root_pct: 77.4,
						children: [
							{
								account_id: "Expenses:Housing:Rent",
								name: "Rent",
								kind: "expense",
								total_minor: 120_000,
								share_of_parent_pct: 100,
								share_of_root_pct: 77.4,
								children: [],
							},
						],
					},
				],
			},
			business: { groupId: "business", months: 6, mode: "monthly_average", totalMinor: 0, nodes: [] },
		};
		const balanceSeriesByAccount: Record<string, DashboardBalanceData> = {
			"Assets:Personal:Monzo": {
				scopeKind: "account",
				scopeId: "Assets:Personal:Monzo",
				scopeLabel: "Personal Monzo",
				series: [
					{ date: "2026-02-27", balance_minor: 110_000 },
					{ date: "2026-02-28", balance_minor: 115_000 },
					{ date: "2026-03-01", balance_minor: 120_000 },
				],
			},
			"Assets:Personal:Investments": {
				scopeKind: "account",
				scopeId: "Assets:Personal:Investments",
				scopeLabel: "Personal Investments",
				series: [
					{ date: "2026-02-27", balance_minor: 480_000 },
					{ date: "2026-03-01", balance_minor: 500_000 },
				],
			},
			"Assets:Business:Monzo": {
				scopeKind: "account",
				scopeId: "Assets:Business:Monzo",
				scopeLabel: "Business Monzo",
				series: [{ date: "2026-03-01", balance_minor: 900_000 }],
			},
		};
		const contributionSeriesByAccount: Record<string, DashboardContributionData> = {
			"Assets:Personal:Investments": {
				accountId: "Assets:Personal:Investments",
				accountLabel: "Personal Investments",
				series: [{ date: "2026-03-01", contributions_minor: 300_000 }],
			},
		};

		const data = await loadDashboardPageData({
			url: new URL("https://fin.test/?group=business"),
			client: createMockClient({
				configShow: async () => configShow,
				health: async () => health,
				viewAccounts: async () => accounts,
				reportSummary: async () => summary,
				reportCashflow: async (group) => cashflowByGroup[group] ?? cashflowByGroup.business,
				reportRunway: async (group) => runwayByGroup[group] ?? runwayByGroup.business,
				dashboardKpis: async (group) => kpisByGroup[group] ?? kpisByGroup.business,
				dashboardAllocation: async (group) => allocationByGroup[group] ?? allocationByGroup.business,
				dashboardFlow: async (group) => flowByGroup[group] ?? flowByGroup.business,
				dashboardHierarchy: async (group) => hierarchyByGroup[group] ?? hierarchyByGroup.business,
				dashboardBalances: async ({ account }) => balanceSeriesByAccount[account ?? ""] ?? balanceSeriesByAccount["Assets:Business:Monzo"],
				dashboardContributions: async (account) => contributionSeriesByAccount[account],
			}),
		});

		expect(data.initialGroup).toBe("business");
		expect(data.config.ui.groupColumnOrder).toEqual(["personal", "business"]);
		expect(data.config.finance.investmentProjectionAnnualReturns).toEqual({ low: 0.04, mid: 0.06, high: 0.08 });
		expect(data.accounts).toHaveLength(3);
		expect(data.groupSummary.personal?.recentAnomalyMonths).toEqual(["2025-12", "2026-02"]);
		expect(data.groupCashflowSeries.personal?.[1]).toEqual({
			month: "2026-02",
			incomeMinor: 200_000,
			expenseMinor: 155_000,
			netMinor: 45_000,
			savingsRatePct: 22.5,
			rollingMedianExpenseMinor: 75_000,
			expenseDeviationRatio: 1.1,
		});
		expect(data.groupRunway.business).toEqual({
			runwayMonths: 16.5,
			isNetPositive: true,
			medianExpenseMinor: 210_000,
		});
		expect(data.groupAllocationSnapshots.personal?.dashboard.availableMinor).toBe(180_000);
		expect(data.groupCashFlowData.personal).toEqual({
			nodes: [
				{ name: "Salary", category: "income" },
				{ name: "Personal Monzo", category: "asset" },
				{ name: "Rent", category: "expense" },
			],
			links: [
				{ source: "Salary", target: "Personal Monzo", value: 200_000 },
				{ source: "Personal Monzo", target: "Rent", value: 120_000 },
			],
		});
		expect(data.groupExpenseHierarchy.personal?.[0]).toEqual({
			accountId: "Expenses:Housing",
			name: "Housing",
			kind: "expense",
			totalMinor: 120_000,
			shareOfParentPct: 100,
			shareOfRootPct: 77.4,
			children: [
				{
					accountId: "Expenses:Housing:Rent",
					name: "Rent",
					kind: "expense",
					totalMinor: 120_000,
					shareOfParentPct: 100,
					shareOfRootPct: 77.4,
					children: [],
				},
			],
		});
		expect(data.accountBalanceSeries["Assets:Personal:Monzo"]?.at(-1)).toEqual({ date: "2026-03-01", balanceMinor: 120_000 });
		expect(data.accountContributionSeries["Assets:Personal:Investments"]?.[0]).toEqual({
			date: "2026-03-01",
			contributionsMinor: 300_000,
		});
	});

	test("returns empty dashboard structures when fin-api is unavailable", async () => {
		const data = await loadDashboardPageData({
			url: new URL("https://fin.test/"),
			client: createMockClient({
				configShow: async () => {
					throw new Error("connect ENOENT");
				},
				health: async () => {
					throw new Error("connect ENOENT");
				},
			}),
		});

		expect(data.initialGroup).toBe("personal");
		expect(data.availableGroups).toEqual(["personal", "joint", "business"]);
		expect(data.accounts).toEqual([]);
		expect(data.groupCashFlowData.personal).toEqual({ nodes: [], links: [] });
		expect(data.connection.error).toBe("api unavailable");
	});
});
