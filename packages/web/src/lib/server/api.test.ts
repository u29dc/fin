import { describe, expect, test } from "bun:test";

import {
	FinApiError,
	createFinApiClient,
	loadShellState,
	resolveApiTransport,
	resolveFinHome,
	type ApiEnvelope,
	type ConfigShowData,
	type HealthReport,
} from "$lib/server/api";

function jsonResponse<T>(payload: ApiEnvelope<T>, status = 200): Response {
	return new Response(JSON.stringify(payload), {
		status,
		headers: { "content-type": "application/json" },
	});
}

describe("resolveFinHome", () => {
	test("prefers FIN_HOME then TOOLS_HOME then HOME", () => {
		expect(resolveFinHome({ FIN_HOME: "/tmp/fin-home" })).toBe("/tmp/fin-home");
		expect(resolveFinHome({ TOOLS_HOME: "/tmp/tools" })).toBe("/tmp/tools/fin");
		expect(resolveFinHome({ HOME: "/tmp/home" })).toBe("/tmp/home/.tools/fin");
	});
});

describe("resolveApiTransport", () => {
	test("defaults to unix socket transport", () => {
		expect(resolveApiTransport({ FIN_HOME: "/tmp/fin-home" })).toEqual({
			kind: "unix",
			origin: "http://localhost",
			socketPath: "/tmp/fin-home/run/fin-api.sock",
		});
	});

	test("uses tcp transport when FIN_API_BASE_URL is set", () => {
		expect(resolveApiTransport({ FIN_API_BASE_URL: "http://127.0.0.1:7414/" })).toEqual({
			kind: "tcp",
			origin: "http://127.0.0.1:7414",
		});
	});
});

describe("createFinApiClient", () => {
	test("sends unix fetch options by default", async () => {
		const calls: Array<{ url: string; init: RequestInit & { unix?: string } }> = [];
		const client = createFinApiClient({
			env: { FIN_HOME: "/tmp/fin-home" },
			fetch: async (url, init) => {
				calls.push({ url: String(url), init: init ?? {} });
				return jsonResponse<ConfigShowData>({
					ok: true,
					data: {
						groups: [],
						accounts: {},
						financial: {},
						reserves: { defaultMode: "conservative", modes: {}, groups: {} },
						configPath: "/tmp/fin-home/data/fin.config.toml",
					},
					meta: { tool: "config.show", elapsed: 1 },
				});
			},
		});

		await client.configShow();
		expect(calls).toHaveLength(1);
		expect(calls[0]).toEqual({
			url: "http://localhost/v1/config/show",
			init: { unix: "/tmp/fin-home/run/fin-api.sock" },
		});
	});

	test("throws FinApiError for API envelopes with ok=false", async () => {
		const client = createFinApiClient({
			env: { FIN_API_BASE_URL: "http://127.0.0.1:7414" },
			fetch: async () =>
				jsonResponse(
					{
						ok: false,
						error: {
							code: "NO_CONFIG",
							message: "Config file not found",
							hint: "Create fin.config.toml",
						},
						meta: { tool: "config.show", elapsed: 1 },
					},
					503,
				),
		});

		await expect(client.configShow()).rejects.toBeInstanceOf(FinApiError);
	});

	test("uses camelCase query keys for dashboard endpoints", async () => {
		const calls: string[] = [];
		const client = createFinApiClient({
			env: { FIN_API_BASE_URL: "http://127.0.0.1:7414" },
			fetch: async (url) => {
				calls.push(String(url));
				return jsonResponse({
					ok: true,
					data: {
						scopeKind: "account",
						scopeId: "Assets:Personal:Monzo",
						scopeLabel: "Personal Monzo",
						series: [],
					},
					meta: { tool: "dashboard.balances", elapsed: 1 },
				});
			},
		});

		await client.dashboardBalances({
			account: "Assets:Personal:Monzo",
			downsampleMinStepDays: 14,
			limit: 90,
		});

		expect(calls[0]).toContain("downsampleMinStepDays=14");
		expect(calls[0]).not.toContain("downsample_min_step_days");
	});

	test("uses camelCase projection query keys", async () => {
		const calls: string[] = [];
		const client = createFinApiClient({
			env: { FIN_API_BASE_URL: "http://127.0.0.1:7414" },
			fetch: async (url) => {
				calls.push(String(url));
				return jsonResponse({
					ok: true,
					data: {
						groups: ["personal", "business"],
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
					},
					meta: { tool: "dashboard.projection", elapsed: 1 },
				});
			},
		});

		await client.dashboardProjection({
			consolidated: true,
			include: "personal,business",
			minimumBurnRatio: 0.75,
			asOf: "2026-03-01",
			trailingOutflowWindowMonths: 18,
			months: 36,
		});

		expect(calls[0]).toContain("minimumBurnRatio=0.75");
		expect(calls[0]).toContain("asOf=2026-03-01");
		expect(calls[0]).toContain("trailingOutflowWindowMonths=18");
		expect(calls[0]).not.toContain("minimum_burn_ratio");
		expect(calls[0]).not.toContain("as_of");
		expect(calls[0]).not.toContain("trailing_outflow_window_months");
	});

	test("sends transaction paging queries and detail paths", async () => {
		const calls: string[] = [];
		const client = createFinApiClient({
			env: { FIN_API_BASE_URL: "http://127.0.0.1:7414" },
			fetch: async (url) => {
				calls.push(String(url));
				if (String(url).includes("/v1/view/transactions/")) {
					return jsonResponse({
						ok: true,
						data: {
							posting_id: "posting-1",
							journal_entry_id: "entry-1",
							chart_account_id: "Assets:Personal:Monzo",
							posted_at: "2026-03-01T12:00:00Z",
							posted_date: "2026-03-01",
							amount_minor: -12345,
							currency: "GBP",
							description: "Coffee",
							raw_description: "STARBUCKS",
							clean_description: "Coffee",
							counterparty: "Starbucks",
							source_file: "monzo.csv",
							is_transfer: false,
							pair_postings: [],
						},
						meta: { tool: "view.transactions", elapsed: 1 },
					});
				}
				return jsonResponse({
					ok: true,
					data: {
						items: [],
						count: 0,
						totalCount: 0,
						hasMore: false,
						nextCursor: null,
						nextCursorToken: null,
					},
					meta: { tool: "view.transactions", elapsed: 1, count: 0, total: 0, hasMore: false },
				});
			},
		});

		await client.viewTransactions({
			group: "personal",
			search: "coffee",
			limit: 100,
			sortField: "description",
			sortDirection: "asc",
			after: "opaque-token",
		});
		await client.viewTransactionDetail("posting:1");

		expect(calls[0]).toContain("/v1/view/transactions?");
		expect(calls[0]).toContain("group=personal");
		expect(calls[0]).toContain("search=coffee");
		expect(calls[0]).toContain("limit=100");
		expect(calls[0]).toContain("sortField=description");
		expect(calls[0]).toContain("sortDirection=asc");
		expect(calls[0]).toContain("after=opaque-token");
		expect(calls[1]).toContain("/v1/view/transactions/posting%3A1");
	});
});

describe("loadShellState", () => {
	const configShow: ConfigShowData = {
		groups: [
			{ id: "business", label: "Business", icon: "briefcase", taxType: "corp", expenseReserveMonths: 2, defaultReserveMode: "recurring" },
			{ id: "personal", label: "Personal", icon: "user", taxType: "income", expenseReserveMonths: 3, defaultReserveMode: "conservative" },
		],
		accounts: {},
		financial: {},
		reserves: { defaultMode: "recurring", modes: {}, groups: {} },
		configPath: "/tmp/fin.config.toml",
	};
	const blockedHealth: HealthReport = {
		status: "blocked",
		checks: [
			{ id: "db_schema", label: "Database schema", status: "invalid", severity: "blocking", detail: "version 5, expected 6", fix: ["fin import"] },
		],
		summary: { ok: 0, blocking: 1, degraded: 0 },
	};

    test("uses config groups while surfacing non-ready health", async () => {
		let callIndex = 0;
		const client = createFinApiClient({
			env: { FIN_API_BASE_URL: "http://127.0.0.1:7414" },
			fetch: async () => {
				callIndex += 1;
				if (callIndex === 1) {
					return jsonResponse({ ok: true, data: configShow, meta: { tool: "config.show", elapsed: 1 } });
				}
				return jsonResponse({ ok: true, data: blockedHealth, meta: { tool: "health", elapsed: 1 } });
			},
		});

        const shell = await loadShellState(client);
        expect(shell.availableGroups).toEqual(["business", "personal"]);
        expect(shell.groupMetadata.personal?.icon).toBe("user");
        expect(shell.connection.error).toBe("api blocked");
        expect(shell.connection.detail).toContain("Database schema");
    });

	test("falls back to placeholder groups when the API is unavailable", async () => {
		const client = createFinApiClient({
			env: { FIN_HOME: "/tmp/fin-home" },
			fetch: async () => {
				throw new Error("connect ENOENT /tmp/fin-home/run/fin-api.sock");
			},
		});

		const shell = await loadShellState(client);
		expect(shell.availableGroups).toEqual(["personal", "joint", "business"]);
		expect(shell.connection.error).toBe("api unavailable");
		expect(shell.connection.detail).toContain("ENOENT");
	});
});
