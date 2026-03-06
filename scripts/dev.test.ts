import { describe, expect, test } from "bun:test";

import {
	buildTcpBindAddr,
	buildWebEnv,
	resolveApiDevTarget,
	resolveFinHome,
	shouldSuppressApiLogLine,
} from "./dev";

describe("resolveFinHome", () => {
	test("prefers FIN_HOME then TOOLS_HOME then HOME", () => {
		expect(resolveFinHome({ FIN_HOME: "/tmp/fin-home" })).toBe("/tmp/fin-home");
		expect(resolveFinHome({ TOOLS_HOME: "/tmp/tools" })).toBe("/tmp/tools/fin");
		expect(resolveFinHome({ HOME: "/tmp/home" })).toBe("/tmp/home/.tools/fin");
	});
});

describe("resolveApiDevTarget", () => {
	test("defaults to unix socket orchestration", () => {
		const target = resolveApiDevTarget({ FIN_HOME: "/tmp/fin-home" });
		expect(target.kind).toBe("unix");
		if (target.kind !== "unix") {
			throw new Error("expected unix target");
		}
		expect(target.socketPath).toBe("/tmp/fin-home/run/fin-api.sock");
		expect(target.apiArgs).toEqual([
			"run",
			"-q",
			"-p",
			"fin-api",
			"--",
			"start",
			"--transport",
			"unix",
			"--socket-path",
			"/tmp/fin-home/run/fin-api.sock",
		]);
		expect(target.webEnv.FIN_API_SOCKET_PATH).toBe("/tmp/fin-home/run/fin-api.sock");
		expect(target.webEnv.FIN_API_BASE_URL).toBeUndefined();
	});

	test("uses tcp orchestration when FIN_API_BASE_URL is set", () => {
		const target = resolveApiDevTarget({ FIN_API_BASE_URL: "http://127.0.0.1:7811/" });
		expect(target.kind).toBe("tcp");
		if (target.kind !== "tcp") {
			throw new Error("expected tcp target");
		}
		expect(target.origin).toBe("http://127.0.0.1:7811");
		expect(target.bindAddr).toBe("127.0.0.1:7811");
		expect(target.apiArgs).toEqual([
			"run",
			"-q",
			"-p",
			"fin-api",
			"--",
			"start",
			"--transport",
			"tcp",
			"--tcp-addr",
			"127.0.0.1:7811",
		]);
		expect(target.webEnv.FIN_API_TRANSPORT).toBe("tcp");
		expect(target.webEnv.FIN_API_BASE_URL).toBe("http://127.0.0.1:7811");
		expect(target.webEnv.FIN_API_SOCKET_PATH).toBeUndefined();
	});
});

describe("buildTcpBindAddr", () => {
	test("derives a bind address from the origin", () => {
		expect(buildTcpBindAddr("http://127.0.0.1:7414")).toBe("127.0.0.1:7414");
		expect(buildTcpBindAddr("http://localhost")).toBe("localhost:80");
	});
});

describe("buildWebEnv", () => {
	test("clears conflicting api variables when targeting unix sockets", () => {
		const target = resolveApiDevTarget({ FIN_HOME: "/tmp/fin-home" });
		const env = buildWebEnv(
			{
				FIN_API_BASE_URL: "http://127.0.0.1:9999",
				FIN_API_TRANSPORT: "tcp",
				FIN_API_SOCKET_PATH: "/tmp/old.sock",
			},
			target,
		);
		expect(env.FIN_API_BASE_URL).toBeUndefined();
		expect(env.FIN_API_TRANSPORT).toBeUndefined();
		expect(env.FIN_API_SOCKET_PATH).toBe("/tmp/fin-home/run/fin-api.sock");
	});
});

describe("shouldSuppressApiLogLine", () => {
	test("suppresses routine lifecycle lines while preserving real errors", () => {
		expect(shouldSuppressApiLogLine("fin-api starting | transport=unix | runtime=deferred")).toBeTrue();
		expect(shouldSuppressApiLogLine("fin-api listening | transport=unix | socket=/tmp/fin-api.sock")).toBeTrue();
		expect(shouldSuppressApiLogLine("fin-api stopped")).toBeTrue();
		expect(shouldSuppressApiLogLine("bind fin-api unix socket at /tmp/fin-api.sock")).toBeFalse();
	});
});
