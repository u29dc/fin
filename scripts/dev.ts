import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const API_READY_TIMEOUT_MS = 45_000;
const API_POLL_INTERVAL_MS = 150;
const CHILD_SHUTDOWN_TIMEOUT_MS = 5_000;
const DEFAULT_UNIX_ORIGIN = "http://localhost";
const DEFAULT_TCP_ORIGIN = "http://127.0.0.1:7414";
const DEV_PREFIX = "fin dev";
const WEB_DEV_URL = "http://127.0.0.1:3000";

type EnvLike = Record<string, string | undefined>;

type UnixDevTarget = {
	kind: "unix";
	origin: string;
	socketPath: string;
	endpointLabel: string;
	apiArgs: string[];
	webEnv: EnvLike;
	requestInit: RequestInit & { unix: string };
};

type TcpDevTarget = {
	kind: "tcp";
	origin: string;
	bindAddr: string;
	endpointLabel: string;
	apiArgs: string[];
	webEnv: EnvLike;
	requestInit: RequestInit;
};

export type ApiDevTarget = UnixDevTarget | TcpDevTarget;

export type HealthProbe = {
	reachable: boolean;
	status: string | null;
};

function log(message: string): void {
	console.error(`${DEV_PREFIX} | ${message}`);
}

function normalizeUrl(value: string | undefined): string | null {
	if (!value) {
		return null;
	}
	const trimmed = value.trim();
	if (trimmed.length === 0) {
		return null;
	}
	return trimmed.endsWith("/") ? trimmed.slice(0, -1) : trimmed;
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

export function buildTcpBindAddr(origin: string): string {
	const url = new URL(origin);
	const port = url.port || (url.protocol === "https:" ? "443" : "80");
	return `${url.hostname}:${port}`;
}

export function buildWebEnv(baseEnv: EnvLike, target: ApiDevTarget): EnvLike {
	const env: EnvLike = { ...baseEnv };
	delete env.FIN_API_BASE_URL;
	delete env.FIN_API_ORIGIN;
	delete env.FIN_API_SOCKET_PATH;
	delete env.FIN_API_TRANSPORT;

	if (target.kind === "unix") {
		env.FIN_API_ORIGIN = target.origin;
		env.FIN_API_SOCKET_PATH = target.socketPath;
		return env;
	}

	env.FIN_API_TRANSPORT = "tcp";
	env.FIN_API_BASE_URL = target.origin;
	return env;
}

export function resolveApiDevTarget(env: EnvLike = process.env): ApiDevTarget {
	const explicitTransport = env.FIN_API_TRANSPORT?.trim().toLowerCase();
	const explicitBaseUrl = normalizeUrl(env.FIN_API_BASE_URL);
	if (explicitTransport === "tcp" || explicitBaseUrl) {
		const origin = explicitBaseUrl ?? DEFAULT_TCP_ORIGIN;
		const bindAddr = buildTcpBindAddr(origin);
		const target: TcpDevTarget = {
			kind: "tcp",
			origin,
			bindAddr,
			endpointLabel: origin,
			apiArgs: ["run", "-q", "-p", "fin-api", "--", "start", "--transport", "tcp", "--tcp-addr", bindAddr],
			webEnv: buildWebEnv(env, {
				kind: "tcp",
				origin,
				bindAddr,
				endpointLabel: origin,
				apiArgs: [],
				webEnv: {},
				requestInit: {},
			}),
			requestInit: {},
		};
		return target;
	}

	const socketPath = env.FIN_API_SOCKET_PATH ?? path.join(resolveFinHome(env), "run", "fin-api.sock");
	const origin = normalizeUrl(env.FIN_API_ORIGIN) ?? DEFAULT_UNIX_ORIGIN;
	const target: UnixDevTarget = {
		kind: "unix",
		origin,
		socketPath,
		endpointLabel: `unix socket ${socketPath}`,
		apiArgs: ["run", "-q", "-p", "fin-api", "--", "start", "--transport", "unix", "--socket-path", socketPath],
		webEnv: buildWebEnv(env, {
			kind: "unix",
			origin,
			socketPath,
			endpointLabel: `unix socket ${socketPath}`,
			apiArgs: [],
			webEnv: {},
			requestInit: { unix: socketPath },
		}),
		requestInit: { unix: socketPath },
	};
	return target;
}

export function shouldSuppressApiLogLine(line: string): boolean {
	return (
		line.startsWith("fin-api starting |") ||
		line.startsWith("fin-api listening |") ||
		line === "fin-api stopped"
	);
}

function cargoBinary(): string {
	return Bun.which("cargo") ?? "cargo";
}

function bunBinary(): string {
	return Bun.which("bun") ?? process.execPath;
}

function signalProcessTree(processHandle: Bun.Subprocess, signal: NodeJS.Signals): void {
	if (process.platform !== "win32") {
		try {
			process.kill(-processHandle.pid, signal);
			return;
		} catch {
			// Fall through to direct child signaling.
		}
	}
	processHandle.kill(signal);
}

function isNonEmptyLine(line: string): boolean {
	return line.trim().length > 0;
}

async function forwardLines(
	stream: ReadableStream<Uint8Array> | null,
	onLine: (line: string) => void,
): Promise<void> {
	if (!stream) {
		return;
	}

	const reader = stream.pipeThrough(new TextDecoderStream()).getReader();
	let buffer = "";
	try {
		while (true) {
			const { value, done } = await reader.read();
			if (done) {
				break;
			}
			buffer += value;
			while (true) {
				const newlineIndex = buffer.indexOf("\n");
				if (newlineIndex === -1) {
					break;
				}
				const line = buffer.slice(0, newlineIndex).replace(/\r$/, "");
				buffer = buffer.slice(newlineIndex + 1);
				if (isNonEmptyLine(line)) {
					onLine(line);
				}
			}
		}
	} finally {
		const trailing = buffer.replace(/\r$/, "");
		if (isNonEmptyLine(trailing)) {
			onLine(trailing);
		}
		reader.releaseLock();
	}
}

async function probeHealth(target: ApiDevTarget): Promise<HealthProbe> {
	try {
		const response = await fetch(new URL("/v1/health", target.origin), target.requestInit);
		if (!response.ok) {
			return { reachable: false, status: null };
		}
		const payload = (await response.json()) as {
			ok?: boolean;
			data?: { status?: string };
		};
		return {
			reachable: true,
			status: payload.ok && payload.data?.status ? payload.data.status : null,
		};
	} catch {
		return { reachable: false, status: null };
	}
}

async function waitForApiReady(apiProcess: Bun.Subprocess, target: ApiDevTarget): Promise<HealthProbe> {
	let exitCode: number | null = null;
	void apiProcess.exited.then((code) => {
		exitCode = code;
	});

	const deadline = Date.now() + API_READY_TIMEOUT_MS;
	let lastProbe: HealthProbe = { reachable: false, status: null };
	while (Date.now() < deadline) {
		if (exitCode !== null) {
			throw new Error(`fin-api exited before becoming ready (code ${exitCode})`);
		}
		lastProbe = await probeHealth(target);
		if (lastProbe.reachable) {
			return lastProbe;
		}
		await delay(API_POLL_INTERVAL_MS);
	}

	throw new Error(`timed out waiting for fin-api readiness at ${target.endpointLabel}; last status ${lastProbe.status ?? "unreachable"}`);
}

async function terminateChild(processHandle: Bun.Subprocess | null, label: string): Promise<void> {
	if (!processHandle) {
		return;
	}
	const alreadyExited = await Promise.race([
		processHandle.exited.then(() => true),
		delay(0).then(() => false),
	]);
	if (alreadyExited) {
		return;
	}

	try {
		signalProcessTree(processHandle, "SIGTERM");
	} catch {
		return;
	}

	const exited = await Promise.race([
		processHandle.exited.then(() => true),
		delay(CHILD_SHUTDOWN_TIMEOUT_MS).then(() => false),
	]);
	if (exited) {
		return;
	}

	log(`${label} did not exit after SIGTERM; forcing shutdown`);
	try {
		signalProcessTree(processHandle, "SIGKILL");
	} catch {
		return;
	}
	await processHandle.exited;
}

export async function runDev(env: EnvLike = process.env): Promise<void> {
	const repoRoot = path.resolve(import.meta.dir, "..");
	const target = resolveApiDevTarget(env);
	log(`starting fin-api | ${target.endpointLabel}`);

	const apiProcess = Bun.spawn([cargoBinary(), ...target.apiArgs], {
		cwd: repoRoot,
		env: { ...process.env, ...env },
		stdin: "ignore",
		stdout: "pipe",
		stderr: "pipe",
		detached: true,
	});

	const apiLogTasks = [
		forwardLines(apiProcess.stdout, (line) => {
			if (!shouldSuppressApiLogLine(line)) {
				console.error(`[fin-api] ${line}`);
			}
		}),
		forwardLines(apiProcess.stderr, (line) => {
			if (!shouldSuppressApiLogLine(line)) {
				console.error(`[fin-api] ${line}`);
			}
		}),
	];

	let webProcess: Bun.Subprocess | null = null;
	let cleaningUp = false;
	const cleanup = async (reason: string): Promise<void> => {
		if (cleaningUp) {
			return;
		}
		cleaningUp = true;
		log(`stopping dev stack | ${reason}`);
		await Promise.allSettled([
			terminateChild(webProcess, "web dev server"),
			terminateChild(apiProcess, "fin-api"),
		]);
	};

	let signalReason = "";
	const handleSignal = (signal: NodeJS.Signals) => {
		signalReason = signal;
		void cleanup(signal);
	};
	process.once("SIGINT", handleSignal);
	process.once("SIGTERM", handleSignal);

	try {
		const health = await waitForApiReady(apiProcess, target);
		const healthLabel = health.status ? ` | health=${health.status}` : "";
		log(`fin-api ready${healthLabel} | ${target.endpointLabel}`);
		log(`starting web dev server | ${WEB_DEV_URL}`);

			webProcess = Bun.spawn([bunBinary(), "run", "--filter", "@fin/web", "dev", "--", "--host", "127.0.0.1"], {
				cwd: repoRoot,
				env: buildWebEnv({ ...process.env, ...env }, target),
				stdin: "inherit",
				stdout: "inherit",
				stderr: "inherit",
				detached: true,
			});

		const result = await Promise.race([
			apiProcess.exited.then((code) => ({ winner: "api" as const, code })),
			webProcess.exited.then((code) => ({ winner: "web" as const, code })),
		]);

		if (signalReason) {
			await cleanup(signalReason);
			return;
		}

		await cleanup(`${result.winner} exited with code ${result.code}`);
		if (result.winner === "api") {
			throw new Error(`fin-api exited unexpectedly with code ${result.code}`);
		}
		if (result.code !== 0) {
			throw new Error(`web dev server exited with code ${result.code}`);
		}
	} finally {
		await cleanup(signalReason || "orchestrator exit");
		process.removeListener("SIGINT", handleSignal);
		process.removeListener("SIGTERM", handleSignal);
		await Promise.allSettled(apiLogTasks);
	}
}

if (import.meta.main) {
	try {
		await runDev();
	} catch (error) {
		const detail = error instanceof Error ? error.message : String(error);
		console.error(`${DEV_PREFIX} | ${detail}`);
		process.exit(1);
	}
}
