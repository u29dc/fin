# fin-api Operator Notes

## Purpose

`fin-api` is the local read-only daemon for `fin`.

- Default transport: HTTP plus JSON over a Unix domain socket.
- Default socket path: `$FIN_HOME/run/fin-api.sock`
- Fallback transport: loopback TCP for portability or debugging.
- Contract shape: every non-probe response uses `ok`, `data` or `error`, and `meta`.
- Timing field: `meta.elapsed` is server-side handler time in milliseconds, not full page-render time.

## Start

Default Unix socket:

```bash
cargo run -p fin-api -- start --check-runtime
```

Explicit runtime paths:

```bash
cargo run -p fin-api -- start \
  --config-path "$FIN_HOME/data/fin.config.toml" \
  --db-path "$FIN_HOME/data/fin.db" \
  --socket-path "$FIN_HOME/run/fin-api.sock"
```

Loopback TCP fallback:

```bash
cargo run -p fin-api -- start \
  --transport tcp \
  --tcp-addr 127.0.0.1:7414 \
  --config-path "$FIN_HOME/data/fin.config.toml" \
  --db-path "$FIN_HOME/data/fin.db"
```

## Probe And Health

Unix socket probe:

```bash
curl --silent --unix-socket "$FIN_HOME/run/fin-api.sock" http://localhost/__probe
```

Unix socket health:

```bash
curl --silent --unix-socket "$FIN_HOME/run/fin-api.sock" http://localhost/v1/health | jq
```

TCP health:

```bash
curl --silent http://127.0.0.1:7414/v1/health | jq
```

## Endpoint Families

- Orientation: `/v1/version`, `/v1/tools`, `/v1/tools/{name}`, `/v1/health`
- Runtime views: `/v1/config/*`, `/v1/rules/*`, `/v1/sanitize/discover`, `/v1/view/*`
- Reports: `/v1/report/cashflow`, `/v1/report/health`, `/v1/report/runway`, `/v1/report/reserves`, `/v1/report/categories`, `/v1/report/audit`, `/v1/report/summary`
- Dashboard surfaces: `/v1/dashboard/kpis`, `/v1/dashboard/allocation`, `/v1/dashboard/hierarchy`, `/v1/dashboard/flow`, `/v1/dashboard/balances`, `/v1/dashboard/contributions`, `/v1/dashboard/projection`

## Error Semantics

Common error codes:

- `NO_CONFIG`: runtime config file missing
- `INVALID_CONFIG`: config exists but is unreadable or invalid
- `NO_RULES`: rules file missing
- `INVALID_RULES`: rules file exists but is invalid
- `INVALID_INPUT`: query parameters are malformed or mutually exclusive
- `NOT_FOUND`: requested group, account, or route does not exist
- `DB_ERROR`, `IO_ERROR`, `RUNTIME_ERROR`: server-side operational failures

Blocked prerequisite errors return HTTP `503`.
Invalid request errors return HTTP `400`.
Missing route or missing scoped entity errors return HTTP `404`.

## Operational Rules

- Keep `fin-api` thin. New read behavior belongs in `fin-sdk`.
- Prefer the Unix socket in local automation and the web server-side client.
- Use TCP only when Unix sockets are unavailable or when debugging from tools that do not speak UDS.
- Treat `meta.elapsed` as handler execution time only. Measure HTTP client overhead separately.

## Troubleshooting

Missing or invalid runtime:

- Run `GET /v1/health`.
- Verify `$FIN_HOME/data/fin.config.toml`.
- Verify `$FIN_HOME/data/fin.db`.
- Verify rules path resolution from config or `$FIN_HOME/data/fin.rules.json`.

Stale socket:

- `fin-api` removes stale socket files before bind.
- If startup still fails, verify the socket path does not already point to a non-socket file.

Request shape problems:

- Use `GET /v1/tools` for CLI-parity read surfaces.
- For dashboard routes, validate mutual exclusivity such as `group` plus `account` on `/v1/dashboard/balances`.

Performance checks:

- Run `bun run util:bench:api`.
- Read [`api-007-fin-api.md`](/Users/han/Git/fin/docs/benchmarks/api-007-fin-api.md) for recorded warm request and handler timings on the fixture dataset.
