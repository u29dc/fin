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
					data: { groups: [], accounts: {}, financial: {}, configPath: "/tmp/fin-home/data/fin.config.toml" },
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
});

describe("loadShellState", () => {
	const configShow: ConfigShowData = {
		groups: [
			{ id: "business", label: "Business", icon: "briefcase", taxType: "corp", expenseReserveMonths: 2 },
			{ id: "personal", label: "Personal", icon: "user", taxType: "income", expenseReserveMonths: 3 },
		],
		accounts: {},
		financial: {},
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
		expect(shell.availableGroups).toEqual(["personal", "business"]);
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
