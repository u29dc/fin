import { describe, expect, test } from "bun:test";

import { loadTransactionsPageData } from "$lib/server/transactions";
import type {
	ConfigShowData,
	FinApiClient,
	HealthReport,
	TransactionDetailData,
	ViewTransactionsData,
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
		viewTransactions: async () => ({
			items: [],
			count: 0,
			totalCount: 0,
			hasMore: false,
			nextCursor: null,
			nextCursorToken: null,
		}),
		viewTransactionDetail: async () => {
			throw new Error("viewTransactionDetail not implemented");
		},
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

describe("loadTransactionsPageData", () => {
	test("maps paginated transactions and selected detail from fin-api", async () => {
		const configShow: ConfigShowData = {
			groups: [
				{ id: "business", label: "Business", icon: "briefcase", taxType: "corp", expenseReserveMonths: 12 },
				{ id: "personal", label: "Personal", icon: "user", taxType: "income", expenseReserveMonths: 6 },
			],
			accounts: {
				business: [{ id: "Assets:Business:Monzo", label: "Business Monzo", provider: "monzo" }],
				personal: [{ id: "Assets:Personal:Monzo", label: "Personal Monzo", provider: "monzo" }],
			},
			financial: {},
			configPath: "/tmp/fin.config.toml",
		};
		const health: HealthReport = {
			status: "ready",
			checks: [],
			summary: { ok: 2, blocking: 0, degraded: 0 },
		};
		const pagePayload: ViewTransactionsData = {
			items: [
				{
					posting_id: "posting-2",
					journal_entry_id: "entry-2",
					chart_account_id: "Assets:Business:Monzo",
					pair_account_ids: ["Expenses:Business:Software"],
					posted_at: "2026-03-04T09:00:00Z",
					posted_date: "2026-03-04",
					amount_minor: -12900,
					currency: "GBP",
					raw_description: "Linear",
					clean_description: "Linear",
					counterparty: "Linear",
				},
			],
			count: 1,
			totalCount: 245,
			hasMore: true,
			nextCursor: null,
			nextCursorToken: "token-3",
		};
		const detailPayload: TransactionDetailData = {
			posting_id: "posting-2",
			journal_entry_id: "entry-2",
			chart_account_id: "Assets:Business:Monzo",
			posted_at: "2026-03-04T09:00:00Z",
			posted_date: "2026-03-04",
			amount_minor: -12900,
			currency: "GBP",
			description: "Linear",
			raw_description: "LINEAR LTD",
			clean_description: "Linear",
			counterparty: "Linear",
			source_file: "business.csv",
			is_transfer: false,
			pair_postings: [
				{
					posting_id: "posting-2-other",
					account_id: "Expenses:Business:Software",
					amount_minor: 12900,
					currency: "GBP",
					memo: "seat",
				},
			],
		};
		const transactionQueries: Array<Record<string, string | number | undefined>> = [];
		const detailCalls: string[] = [];
		const client = createMockClient({
			configShow: async () => configShow,
			health: async () => health,
			viewTransactions: async (query) => {
				transactionQueries.push({
					group: query.group,
					search: query.search,
					limit: query.limit,
					sortField: query.sortField,
					sortDirection: query.sortDirection,
					after: query.after,
				});
				return pagePayload;
			},
			viewTransactionDetail: async (postingId) => {
				detailCalls.push(postingId);
				return detailPayload;
			},
		});

		const page = await loadTransactionsPageData({
			url: new URL(
				"https://fin.test/transactions?group=business&sort=amountMinor&dir=asc&search=linear&cursor=token-1&cursor=token-2&selected=posting-2",
			),
			client,
		});

		expect(page.initialGroup).toBe("business");
		expect(page.initialSort).toBe("amountMinor");
		expect(page.initialDir).toBe("asc");
		expect(page.searchQuery).toBe("linear");
		expect(page.list.pageNumber).toBe(3);
		expect(page.list.rangeStart).toBe(201);
		expect(page.list.rangeEnd).toBe(201);
		expect(page.list.nextCursorToken).toBe("token-3");
		expect(page.list.items[0]).toEqual({
			postingId: "posting-2",
			journalEntryId: "entry-2",
			chartAccountId: "Assets:Business:Monzo",
			pairAccountIds: ["Expenses:Business:Software"],
			postedAt: "2026-03-04T09:00:00Z",
			postedDate: "2026-03-04",
			amountMinor: -12900,
			currency: "GBP",
			rawDescription: "Linear",
			cleanDescription: "Linear",
			counterparty: "Linear",
		});
		expect(page.selectedPostingId).toBe("posting-2");
		expect(page.selectedTransaction?.pairPostings[0]?.accountId).toBe("Expenses:Business:Software");
		expect(transactionQueries).toEqual([
			{
				group: "business",
				search: "linear",
				limit: 100,
				sortField: "amount_minor",
				sortDirection: "asc",
				after: "token-2",
			},
		]);
		expect(detailCalls).toEqual(["posting-2"]);
	});

	test("falls back to the first page when a cursor token becomes stale", async () => {
		const configShow: ConfigShowData = {
			groups: [{ id: "personal", label: "Personal", icon: "user", taxType: "income", expenseReserveMonths: 6 }],
			accounts: {
				personal: [{ id: "Assets:Personal:Monzo", label: "Personal Monzo", provider: "monzo" }],
			},
			financial: {},
			configPath: "/tmp/fin.config.toml",
		};
		const pagePayload: ViewTransactionsData = {
			items: [
				{
					posting_id: "posting-1",
					journal_entry_id: "entry-1",
					chart_account_id: "Assets:Personal:Monzo",
					pair_account_ids: ["Expenses:Personal:Food"],
					posted_at: "2026-03-01T10:00:00Z",
					posted_date: "2026-03-01",
					amount_minor: -560,
					currency: "GBP",
					raw_description: "Pret",
					clean_description: "Pret",
					counterparty: "Pret",
				},
			],
			count: 1,
			totalCount: 1,
			hasMore: false,
			nextCursor: null,
			nextCursorToken: null,
		};
		const detailPayload: TransactionDetailData = {
			posting_id: "posting-1",
			journal_entry_id: "entry-1",
			chart_account_id: "Assets:Personal:Monzo",
			posted_at: "2026-03-01T10:00:00Z",
			posted_date: "2026-03-01",
			amount_minor: -560,
			currency: "GBP",
			description: "Pret",
			raw_description: "PRET",
			clean_description: "Pret",
			counterparty: "Pret",
			source_file: "personal.csv",
			is_transfer: false,
			pair_postings: [],
		};
		const seenAfter: Array<string | undefined> = [];
		const client = createMockClient({
			configShow: async () => configShow,
			viewTransactions: async (query) => {
				seenAfter.push(query.after);
				if (query.after) {
					throw new Error("stale cursor");
				}
				return pagePayload;
			},
			viewTransactionDetail: async () => detailPayload,
		});

		const page = await loadTransactionsPageData({
			url: new URL("https://fin.test/transactions?group=personal&cursor=expired-token"),
			client,
		});

		expect(seenAfter).toEqual(["expired-token", undefined]);
		expect(page.list.pageNumber).toBe(1);
		expect(page.list.cursorTrail).toEqual([]);
		expect(page.selectedPostingId).toBe("posting-1");
	});

	test("falls back to placeholder shell data when fin-api is unavailable", async () => {
		const client = createMockClient({
			configShow: async () => {
				throw new Error("connect ENOENT /tmp/fin-home/run/fin-api.sock");
			},
			health: async () => {
				throw new Error("connect ENOENT /tmp/fin-home/run/fin-api.sock");
			},
		});

		const page = await loadTransactionsPageData({
			url: new URL("https://fin.test/transactions"),
			client,
		});

		expect(page.availableGroups).toEqual(["personal", "joint", "business"]);
		expect(page.list.totalCount).toBe(0);
		expect(page.selectedTransaction).toBeNull();
		expect(page.connection.error).toBe("api unavailable");
	});
});
