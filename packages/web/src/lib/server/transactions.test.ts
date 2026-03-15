import { describe, expect, test } from "bun:test";

import {
	fetchTransactionDetail,
	fetchTransactionsDataset,
	filterTransactionItems,
	loadTransactionsPageData,
	TRANSACTION_LIST_LIMIT,
	type TransactionListItem,
} from "$lib/server/transactions";
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
	test("returns shell state and initial URL selections without loading list data", async () => {
		const configShow: ConfigShowData = {
			groups: [
				{ id: "business", label: "Business", icon: "briefcase", taxType: "corp", expenseReserveMonths: 12, defaultReserveMode: "recurring" },
				{ id: "personal", label: "Personal", icon: "user", taxType: "income", expenseReserveMonths: 6, defaultReserveMode: "conservative" },
			],
			accounts: {
				business: [{ id: "Assets:Business:Monzo", label: "Business Monzo", provider: "monzo" }],
				personal: [{ id: "Assets:Personal:Monzo", label: "Personal Monzo", provider: "monzo" }],
			},
			financial: {},
			reserves: { defaultMode: "recurring", modes: {}, groups: {} },
			configPath: "/tmp/fin.config.toml",
		};
		const health: HealthReport = {
			status: "ready",
			checks: [],
			summary: { ok: 2, blocking: 0, degraded: 0 },
		};
		const transactionCalls: Array<Record<string, string | number | undefined>> = [];
		const client = createMockClient({
			configShow: async () => configShow,
			health: async () => health,
			viewTransactions: async (query) => {
				transactionCalls.push({
					group: query.group,
					limit: query.limit,
					sortField: query.sortField,
					sortDirection: query.sortDirection,
				});
				return {
					items: [],
					count: 0,
					totalCount: 0,
					hasMore: false,
					nextCursor: null,
					nextCursorToken: null,
				};
			},
		});

		const page = await loadTransactionsPageData({
			url: new URL("https://fin.test/transactions?group=business&sort=amountMinor&dir=asc&search=linear&selected=posting-2"),
			client,
		});

		expect(page.availableGroups).toEqual(["business", "personal"]);
		expect(page.initialGroup).toBe("business");
		expect(page.initialSort).toBe("amountMinor");
		expect(page.initialDir).toBe("asc");
		expect(page.searchQuery).toBe("linear");
		expect(page.selectedPostingId).toBe("posting-2");
		expect(transactionCalls).toEqual([]);
	});
});

describe("fetchTransactionsDataset", () => {
	test("loads a large sorted dataset without pagination metadata", async () => {
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
			hasMore: false,
			nextCursor: null,
			nextCursorToken: null,
		};
		const transactionQueries: Array<Record<string, string | number | undefined>> = [];
		const client = createMockClient({
			viewTransactions: async (query) => {
				transactionQueries.push({
					group: query.group,
					limit: query.limit,
					sortField: query.sortField,
					sortDirection: query.sortDirection,
					after: query.after,
					search: query.search,
				});
				return pagePayload;
			},
		});

		const list = await fetchTransactionsDataset(client, {
			group: "business",
			sort: "amountMinor",
			direction: "asc",
		});

		expect(transactionQueries).toEqual([
			{
				group: "business",
				limit: TRANSACTION_LIST_LIMIT,
				sortField: "amount_minor",
				sortDirection: "asc",
				after: undefined,
				search: undefined,
			},
		]);
		expect(list).toEqual({
			items: [
				{
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
				},
			],
			loadedCount: 1,
			totalCount: 245,
			limit: TRANSACTION_LIST_LIMIT,
			truncated: true,
		});
	});

	test("maps pair-account sorting to account_id for deterministic account-code ordering", async () => {
		const transactionQueries: Array<Record<string, string | number | undefined>> = [];
		const client = createMockClient({
			viewTransactions: async (query) => {
				transactionQueries.push({
					group: query.group,
					limit: query.limit,
					sortField: query.sortField,
					sortDirection: query.sortDirection,
				});
				return {
					items: [],
					count: 0,
					totalCount: 0,
					hasMore: false,
					nextCursor: null,
					nextCursorToken: null,
				};
			},
		});

		await fetchTransactionsDataset(client, {
			group: "business",
			sort: "pairAccountId",
			direction: "asc",
		});

		expect(transactionQueries).toEqual([
			{
				group: "business",
				limit: TRANSACTION_LIST_LIMIT,
				sortField: "account_id",
				sortDirection: "asc",
			},
		]);
	});
});

describe("fetchTransactionDetail", () => {
	test("maps selected detail payloads from fin-api", async () => {
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
		const detailCalls: string[] = [];
		const client = createMockClient({
			viewTransactionDetail: async (postingId) => {
				detailCalls.push(postingId);
				return detailPayload;
			},
		});

		const detail = await fetchTransactionDetail(client, "posting-2");

		expect(detailCalls).toEqual(["posting-2"]);
		expect(detail.pairPostings[0]?.accountId).toBe("Expenses:Business:Software");
		expect(detail.cleanDescription).toBe("Linear");
	});
});

describe("filterTransactionItems", () => {
	test("matches description, counterparty, account ids, and pair accounts with live multi-term filtering", () => {
		const items: TransactionListItem[] = [
			{
				postingId: "posting-1",
				journalEntryId: "entry-1",
				chartAccountId: "Assets:Business:Monzo",
				pairAccountIds: ["Expenses:Business:Software"],
				postedAt: "2026-03-04T09:00:00Z",
				postedDate: "2026-03-04",
				amountMinor: -12900,
				currency: "GBP",
				rawDescription: "LINEAR LTD",
				cleanDescription: "Linear",
				counterparty: "Linear",
			},
			{
				postingId: "posting-2",
				journalEntryId: "entry-2",
				chartAccountId: "Assets:Personal:Monzo",
				pairAccountIds: ["Expenses:Personal:Groceries"],
				postedAt: "2026-03-01T09:00:00Z",
				postedDate: "2026-03-01",
				amountMinor: -1097,
				currency: "GBP",
				rawDescription: "ASDA",
				cleanDescription: "Asda",
				counterparty: "Asda",
			},
		];

		expect(filterTransactionItems(items, "")).toEqual(items);
		expect(filterTransactionItems(items, "linear")).toEqual([items[0]]);
		expect(filterTransactionItems(items, "personal groceries")).toEqual([items[1]]);
		expect(filterTransactionItems(items, "business software")).toEqual([items[0]]);
		expect(filterTransactionItems(items, "no match")).toEqual([]);
	});
});
