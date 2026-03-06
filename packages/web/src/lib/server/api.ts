import path from "node:path";

import {
	fallbackGroupMetadata,
	fallbackGroups,
	type ConnectionState,
	type GroupId,
	type GroupMeta,
} from "$lib/server/skeleton";

export type ApiMeta = {
	tool: string;
	elapsed: number;
	count?: number;
	total?: number;
	hasMore?: boolean;
};

export type ApiErrorPayload = {
	code: string;
	message: string;
	hint: string;
};

export type ApiSuccessEnvelope<T> = {
	ok: true;
	data: T;
	meta: ApiMeta;
};

export type ApiErrorEnvelope = {
	ok: false;
	error: ApiErrorPayload;
	meta: ApiMeta;
};

export type ApiEnvelope<T> = ApiSuccessEnvelope<T> | ApiErrorEnvelope;

export type ConfigGroup = {
	id: string;
	label: string;
	icon?: string | null;
	taxType: string;
	expenseReserveMonths: number;
};

export type ConfigAccount = {
	id: string;
	provider: string;
	label?: string | null;
	subtype?: string | null;
};

export type ConfigShowData = {
	groups: ConfigGroup[];
	accounts: Record<string, ConfigAccount[]>;
	financial: Record<string, unknown>;
	configPath: string;
};

export type CheckStatus = "ok" | "missing" | "invalid";
export type Severity = "blocking" | "degraded" | "info";
export type HealthStatus = "ready" | "degraded" | "blocked";

export type HealthCheck = {
	id: string;
	label: string;
	status: CheckStatus;
	severity: Severity;
	detail?: string | null;
	fix?: string[] | null;
};

export type HealthSummary = {
	ok: number;
	blocking: number;
	degraded: number;
};

export type HealthReport = {
	status: HealthStatus;
	checks: HealthCheck[];
	summary: HealthSummary;
};

export type ShellState = {
	availableGroups: GroupId[];
	groupMetadata: Record<GroupId, GroupMeta>;
	connection: ConnectionState;
	config: ConfigShowData | null;
	health: HealthReport | null;
};

export type ApiTransport =
	| {
			kind: "unix";
			origin: string;
			socketPath: string;
	  }
	| {
			kind: "tcp";
			origin: string;
	  };

export type EnvLike = Record<string, string | undefined>;

type BunRequestInit = RequestInit & {
	unix?: string;
};

type FetchLike = (input: string | URL | Request, init?: BunRequestInit) => Promise<Response>;

export type FinApiClient = {
	readonly transport: ApiTransport;
	get<T>(pathname: string, query?: Record<string, QueryValue>): Promise<T>;
	configShow(): Promise<ConfigShowData>;
	health(): Promise<HealthReport>;
};

type QueryValue = string | number | boolean | null | undefined;

const DEFAULT_UNIX_ORIGIN = "http://localhost";
const DEFAULT_TCP_ORIGIN = "http://127.0.0.1:7414";
const PREFERRED_GROUP_ORDER = ["personal", "joint", "business"];

export class FinApiError extends Error {
	readonly code: string;
	readonly status: number;
	readonly hint: string;
	readonly meta: ApiMeta | null;

	constructor(message: string, options: { code: string; status: number; hint: string; meta?: ApiMeta | null }) {
		super(message);
		this.name = "FinApiError";
		this.code = options.code;
		this.status = options.status;
		this.hint = options.hint;
		this.meta = options.meta ?? null;
	}
}

export function resolveFinHome(env: EnvLike = process.env): string {
	if (env.FIN_HOME) {
		return env.FIN_HOME;
	}
	if (env.TOOLS_HOME) {
		return path.join(env.TOOLS_HOME, "fin");
	}
	const home = env.HOME ?? process.env.HOME ?? ".";
	return path.join(home, ".tools", "fin");
}

export function resolveApiTransport(env: EnvLike = process.env): ApiTransport {
	const explicitTransport = env.FIN_API_TRANSPORT?.trim().toLowerCase();
	const explicitBaseUrl = normalizeUrl(env.FIN_API_BASE_URL);
	if (explicitTransport === "tcp" || explicitBaseUrl) {
		return {
			kind: "tcp",
			origin: explicitBaseUrl ?? DEFAULT_TCP_ORIGIN,
		};
	}

	return {
		kind: "unix",
		origin: normalizeUrl(env.FIN_API_ORIGIN) ?? DEFAULT_UNIX_ORIGIN,
		socketPath: env.FIN_API_SOCKET_PATH ?? path.join(resolveFinHome(env), "run", "fin-api.sock"),
	};
}

export function createFinApiClient(options?: { env?: EnvLike; fetch?: FetchLike }): FinApiClient {
	const transport = resolveApiTransport(options?.env);
	const fetchImpl: FetchLike = options?.fetch ?? ((input, init) => fetch(input, init));

	async function get<T>(pathname: string, query?: Record<string, QueryValue>): Promise<T> {
		const url = buildUrl(transport, pathname, query);
		const requestInit: BunRequestInit = transport.kind === "unix" ? { unix: transport.socketPath } : {};
		const response = await fetchImpl(url, requestInit);
		const envelope = await parseEnvelope<T>(response);
		if (!response.ok || !envelope.ok) {
			throw toApiError(response.status, envelope);
		}
		return envelope.data;
	}

	return {
		transport,
		get,
		configShow() {
			return get<ConfigShowData>("/v1/config/show");
		},
		health() {
			return get<HealthReport>("/v1/health");
		},
	};
}

export async function loadShellState(client: FinApiClient = createFinApiClient()): Promise<ShellState> {
	const [configResult, healthResult] = await Promise.allSettled([client.configShow(), client.health()]);
	const config = configResult.status === "fulfilled" ? configResult.value : null;
	const health = healthResult.status === "fulfilled" ? healthResult.value : null;
	const shellGroups = deriveGroups(config);

	return {
		availableGroups: shellGroups.availableGroups,
		groupMetadata: shellGroups.groupMetadata,
		connection: deriveConnectionState(client.transport, configResult, healthResult),
		config,
		health,
	};
}

function normalizeUrl(value: string | undefined): string | null {
	if (!value) {
		return null;
	}
	return value.endsWith("/") ? value.slice(0, -1) : value;
}

function buildUrl(transport: ApiTransport, pathname: string, query?: Record<string, QueryValue>): string {
	const url = new URL(pathname, transport.origin);
	for (const [key, value] of Object.entries(query ?? {})) {
		if (value === null || value === undefined || value === "") {
			continue;
		}
		url.searchParams.set(key, String(value));
	}
	return url.toString();
}

async function parseEnvelope<T>(response: Response): Promise<ApiEnvelope<T>> {
	const raw = await response.text();
	try {
		return JSON.parse(raw) as ApiEnvelope<T>;
	} catch (error) {
		throw new FinApiError("fin-api returned invalid JSON", {
			code: "INVALID_RESPONSE",
			status: response.status,
			hint: raw.length > 0 ? raw.slice(0, 200) : "empty response body",
			meta: null,
		});
	}
}

function toApiError(status: number, envelope: ApiEnvelope<unknown>): FinApiError {
	if (envelope.ok) {
		return new FinApiError(`fin-api request failed with status ${status}`, {
			code: "HTTP_ERROR",
			status,
			hint: envelope.meta.tool,
			meta: envelope.meta,
		});
	}
	return new FinApiError(envelope.error.message, {
		code: envelope.error.code,
		status,
		hint: envelope.error.hint,
		meta: envelope.meta,
	});
}

function deriveGroups(config: ConfigShowData | null): Pick<ShellState, "availableGroups" | "groupMetadata"> {
	if (!config) {
		return {
			availableGroups: fallbackGroups,
			groupMetadata: fallbackGroupMetadata,
		};
	}

	const orderedGroups = [...config.groups].sort((left, right) => {
		const leftIndex = preferredGroupIndex(left.id);
		const rightIndex = preferredGroupIndex(right.id);
		return leftIndex - rightIndex || left.label.localeCompare(right.label);
	});

	return {
		availableGroups: orderedGroups.map((group) => group.id),
		groupMetadata: Object.fromEntries(
			orderedGroups.map((group) => [
				group.id,
				{
					label: group.label,
					icon: group.icon ?? "wallet",
				},
			]),
		) as Record<GroupId, GroupMeta>,
	};
}

function preferredGroupIndex(groupId: string): number {
	const index = PREFERRED_GROUP_ORDER.indexOf(groupId);
	return index === -1 ? PREFERRED_GROUP_ORDER.length : index;
}

function deriveConnectionState(
	transport: ApiTransport,
	configResult: PromiseSettledResult<ConfigShowData>,
	healthResult: PromiseSettledResult<HealthReport>,
): ConnectionState {
	const transportLabel = describeTransport(transport);

	if (healthResult.status === "fulfilled") {
		const health = healthResult.value;
		if (health.status === "ready") {
			return {
				loading: false,
				error: null,
				detail: `Connected to fin-api via ${transportLabel}.`,
			};
		}
		return {
			loading: false,
			error: `api ${health.status}`,
			detail: `${summarizeHealthIssues(health)} via ${transportLabel}.`,
		};
	}

	if (configResult.status === "fulfilled") {
		const error = healthResult.reason;
		return {
			loading: false,
			error: "health unavailable",
			detail: `${formatReason(error)} via ${transportLabel}.`,
		};
	}

	const reason = formatReason(configResult.reason);
	return {
		loading: false,
		error: "api unavailable",
		detail: `${reason} via ${transportLabel}.`,
	};
}

function describeTransport(transport: ApiTransport): string {
	if (transport.kind === "tcp") {
		return transport.origin;
	}
	return `unix socket ${transport.socketPath}`;
}

function summarizeHealthIssues(report: HealthReport): string {
	const failingChecks = report.checks.filter((check) => check.status !== "ok");
	if (failingChecks.length === 0) {
		return `Health status ${report.status}`;
	}
	const first = failingChecks[0];
	if (!first) {
		return `Health status ${report.status}`;
	}
	const detail = first.detail ? `: ${first.detail}` : "";
	return `${first.label}${detail}`;
}

function formatReason(reason: unknown): string {
	if (reason instanceof FinApiError) {
		return reason.message;
	}
	if (reason instanceof Error) {
		return reason.message;
	}
	return "Unknown fin-api error";
}
