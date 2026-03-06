import { describe, expect, test } from "bun:test";

import { loadOverviewPageData } from "$lib/server/overview";
import type {
	ConfigShowData,
	DashboardBalanceData,
	DashboardProjectionData,
	FinApiClient,
	HealthReport,
	ViewAccountsData,
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
			scopeKind: "all_assets",
			scopeId: "all-assets",
			scopeLabel: "All assets",
			series: [],
		}),
		dashboardContributions: async () => ({ accountId: "", accountLabel: "", series: [] }),
		dashboardProjection: async () => ({
			groups: [],
			report: {
				scope_kind: "consolidated",
				scope_id: "consolidated",
				liquid_balance_minor: 0,
				current_burn_minor: 0,
				minimum_burn_minor: 0,
				median_monthly_expense_minor: 0,
				thresholds: { warning_minor: null, threshold_minor: null },
				assumptions: {
					as_of_date: "2026-03-01",
					projection_months: 24,
					trailing_outflow_window_months: 12,
					burn_rate_method: "median_outflow",
					minimum_burn_ratio: 0.6,
					full_months_only: true,
					include_as_of_month_in_history: false,
				},
				scenarios: [],
			},
		}),
		...overrides,
	};
}

describe("loadOverviewPageData", () => {
	test("maps all-assets balance history and projection data from fin-api", async () => {
		const configShow: ConfigShowData = {
			groups: [
				{ id: "joint", label: "Joint", icon: "heart", taxType: "none", expenseReserveMonths: 3 },
				{ id: "business", label: "Business", icon: "briefcase", taxType: "corp", expenseReserveMonths: 12 },
				{ id: "personal", label: "Personal", icon: "user", taxType: "income", expenseReserveMonths: 6 },
			],
			accounts: {
				personal: [
					{ id: "Assets:Personal:Monzo", label: "Personal Monzo", provider: "monzo" },
					{ id: "Assets:Personal:Vanguard", label: "Vanguard ISA", provider: "vanguard", subtype: "investment" },
				],
				joint: [{ id: "Assets:Joint:Monzo", label: "Joint Monzo", provider: "monzo" }],
				business: [{ id: "Assets:Business:Monzo", label: "Business Monzo", provider: "monzo" }],
			},
			financial: {},
			configPath: "/tmp/fin.config.toml",
		};
		const health: HealthReport = {
			status: "ready",
			checks: [],
			summary: { ok: 2, blocking: 0, degraded: 0 },
		};
		const viewAccounts: ViewAccountsData = {
			accounts: [
				{
					id: "Assets:Business:Monzo",
					name: "Business Monzo",
					account_type: "asset",
					balance_minor: 1_800_000,
					updated_at: "2026-03-02",
				},
				{
					id: "Assets:Personal:Monzo",
					name: "Personal Monzo",
					account_type: "asset",
					balance_minor: 420_000,
					updated_at: "2026-03-02",
				},
				{
					id: "Assets:Personal:Vanguard",
					name: "Vanguard ISA",
					account_type: "asset",
					balance_minor: 2_600_000,
					updated_at: "2026-03-02",
				},
				{
					id: "Assets:Joint:Monzo",
					name: "Joint Monzo",
					account_type: "asset",
					balance_minor: 780_000,
					updated_at: "2026-03-02",
				},
				{
					id: "Income:Business:Sales",
					name: "Business Sales",
					account_type: "income",
					balance_minor: null,
					updated_at: null,
				},
			],
			total: 5,
		};
		const totalBalances: DashboardBalanceData = {
			scopeKind: "all_assets",
			scopeId: "all-assets",
			scopeLabel: "All assets",
			series: [
				{ date: "2024-01-01", balance_minor: 1_100_000 },
				{ date: "2025-01-01", balance_minor: 2_400_000 },
				{ date: "2026-03-01", balance_minor: 5_600_000 },
			],
		};
		const perAccountBalances: Record<string, DashboardBalanceData> = {
			"Assets:Personal:Monzo": {
				scopeKind: "account",
				scopeId: "Assets:Personal:Monzo",
				scopeLabel: "Personal Monzo",
				series: [
					{ date: "2024-01-01", balance_minor: 150_000 },
					{ date: "2025-01-01", balance_minor: 300_000 },
					{ date: "2026-03-01", balance_minor: 420_000 },
				],
			},
			"Assets:Personal:Vanguard": {
				scopeKind: "account",
				scopeId: "Assets:Personal:Vanguard",
				scopeLabel: "Vanguard ISA",
				series: [
					{ date: "2024-01-01", balance_minor: 500_000 },
					{ date: "2025-01-01", balance_minor: 1_700_000 },
					{ date: "2026-03-01", balance_minor: 2_600_000 },
				],
			},
			"Assets:Joint:Monzo": {
				scopeKind: "account",
				scopeId: "Assets:Joint:Monzo",
				scopeLabel: "Joint Monzo",
				series: [
					{ date: "2024-01-01", balance_minor: 250_000 },
					{ date: "2025-01-01", balance_minor: 520_000 },
					{ date: "2026-03-01", balance_minor: 780_000 },
				],
			},
			"Assets:Business:Monzo": {
				scopeKind: "account",
				scopeId: "Assets:Business:Monzo",
				scopeLabel: "Business Monzo",
				series: [
					{ date: "2024-01-01", balance_minor: 200_000 },
					{ date: "2025-01-01", balance_minor: 780_000 },
					{ date: "2026-03-01", balance_minor: 1_800_000 },
				],
			},
		};
		const projection: DashboardProjectionData = {
			groups: ["personal", "joint", "business"],
			report: {
				scope_kind: "consolidated",
				scope_id: "consolidated",
				liquid_balance_minor: 3_000_000,
				current_burn_minor: 240_000,
				minimum_burn_minor: 144_000,
				median_monthly_expense_minor: 210_000,
				thresholds: { warning_minor: 500_000, threshold_minor: null },
				assumptions: {
					as_of_date: "2026-03-01",
					projection_months: 24,
					trailing_outflow_window_months: 12,
					burn_rate_method: "median_outflow",
					minimum_burn_ratio: 0.6,
					full_months_only: true,
					include_as_of_month_in_history: false,
				},
				scenarios: [
					{
						kind: "current_burn",
						label: "Current burn",
						burn_rate_minor: 240_000,
						is_net_positive: false,
						zero_balance_crossing: { month_index: 13, date: "2027-04-01", balance_minor: 0 },
						warning_crossing: { month_index: 11, date: "2027-02-01", balance_minor: 420_000 },
						threshold_crossing: null,
						points: [
							{ month_index: 0, date: "2026-03-01", balance_minor: 3_000_000 },
							{ month_index: 1, date: "2026-04-01", balance_minor: 2_760_000 },
							{ month_index: 2, date: "2026-05-01", balance_minor: 2_520_000 },
						],
					},
					{
						kind: "minimum_burn",
						label: "Minimum burn",
						burn_rate_minor: 144_000,
						is_net_positive: false,
						zero_balance_crossing: null,
						warning_crossing: null,
						threshold_crossing: null,
						points: [
							{ month_index: 0, date: "2026-03-01", balance_minor: 3_000_000 },
							{ month_index: 1, date: "2026-04-01", balance_minor: 2_856_000 },
							{ month_index: 2, date: "2026-05-01", balance_minor: 2_712_000 },
						],
					},
				],
			},
		};
		const balanceQueries: Array<{ account?: string; downsampleMinStepDays?: number }> = [];

		const client = createMockClient({
			configShow: async () => configShow,
			health: async () => health,
			viewAccounts: async () => viewAccounts,
			dashboardBalances: async (query) => {
				balanceQueries.push({ account: query.account, downsampleMinStepDays: query.downsampleMinStepDays });
				if (!query.account) {
					return totalBalances;
				}
				return perAccountBalances[query.account] ?? {
					scopeKind: "account",
					scopeId: query.account,
					scopeLabel: query.account,
					series: [],
				};
			},
			dashboardProjection: async () => projection,
		});

		const page = await loadOverviewPageData({ client });

		expect(page.availableGroups).toEqual(["personal", "joint", "business"]);
		expect(page.chartAccounts.map((account) => account.id)).toEqual([
			"Assets:Personal:Monzo",
			"Assets:Personal:Vanguard",
			"Assets:Joint:Monzo",
			"Assets:Business:Monzo",
		]);
		expect(page.totalBalanceSeries).toEqual([
			{ date: "2024-01-01", balanceMinor: 1_100_000 },
			{ date: "2025-01-01", balanceMinor: 2_400_000 },
			{ date: "2026-03-01", balanceMinor: 5_600_000 },
		]);
		expect(page.accountBalanceSeries["Assets:Personal:Monzo"]?.at(-1)).toEqual({
			date: "2026-03-01",
			balanceMinor: 420_000,
		});
		expect(page.projection?.thresholds.warningMinor).toBe(500_000);
		expect(page.projection?.thresholds.thresholdMinor).toBeNull();
		expect(page.projection?.currentBurn?.points[1]).toEqual({
			month: 1,
			date: "2026-04-01",
			balanceMinor: 2_760_000,
		});
		expect(page.projection?.currentBurn?.zeroBalanceCrossing?.monthIndex).toBe(13);
		expect(balanceQueries[0]).toEqual({ account: undefined, downsampleMinStepDays: undefined });
		expect(balanceQueries.slice(1)).toEqual([
			{ account: "Assets:Personal:Monzo", downsampleMinStepDays: 7 },
			{ account: "Assets:Personal:Vanguard", downsampleMinStepDays: 7 },
			{ account: "Assets:Joint:Monzo", downsampleMinStepDays: 7 },
			{ account: "Assets:Business:Monzo", downsampleMinStepDays: 7 },
		]);
	});

	test("falls back to an empty overview surface when fin-api is unavailable", async () => {
		const client = createMockClient({
			configShow: async () => {
				throw new Error("connect ENOENT /tmp/fin-home/run/fin-api.sock");
			},
			health: async () => {
				throw new Error("connect ENOENT /tmp/fin-home/run/fin-api.sock");
			},
		});

		const page = await loadOverviewPageData({ client });

		expect(page.availableGroups).toEqual(["personal", "joint", "business"]);
		expect(page.chartAccounts).toEqual([]);
		expect(page.totalBalanceSeries).toEqual([]);
		expect(page.projection).toBeNull();
		expect(page.connection.error).toBe("api unavailable");
	});
});
