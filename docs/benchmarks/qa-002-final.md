# QA-002 Final Validation

This document records the final end-to-end validation for the rewrite roadmap on 2026-03-06.

## Scope

Validated surfaces:

- Rust SDK read models for dashboard, overview, transactions, and reports
- `fin-tui` route layout and interaction smoke coverage
- `fin-api` daemon contract and warm latency targets
- restored SvelteKit web package against `fin-api`
- root `bun run dev` orchestration and cleanup behavior

## Commands

Benchmark commands:

```bash
bun run util:bench:baseline
bun run util:bench:api
```

Manual validation commands:

```bash
FIN_API_BASE_URL='http://127.0.0.1:9' bun run --filter @fin/web dev -- --host 127.0.0.1
FIN_HOME="$(pwd)/target/bench-fixtures/benchmark-runtime" bun run dev
printf 'q' | script -q "$WIDE_LOG" env FIN_HOME="$(pwd)/target/bench-fixtures/benchmark-runtime" COLUMNS=160 LINES=48 cargo run -q -p fin-tui -- >/dev/null
printf '2\033[6~q' | script -q "$NARROW_LOG" env FIN_HOME="$(pwd)/target/bench-fixtures/benchmark-runtime" COLUMNS=100 LINES=32 cargo run -q -p fin-tui -- >/dev/null
```

Quality gate:

```bash
bun run util:check
```

## Final CLI Benchmark Snapshot

Cold one-shot timings from the final QA rerun:

| Target | Real | User | Sys |
| --- | ---: | ---: | ---: |
| `transactions-personal` | `0.03s` | `0.02s` | `0.00s` |
| `cashflow-business` | `0.01s` | `0.01s` | `0.00s` |
| `summary-report` | `0.04s` | `0.03s` | `0.00s` |
| `summary-dashboard` | `0.41s` | `0.07s` | `0.00s` |

Warm repeated timings from the final QA rerun:

| Target | Mean | Std Dev | Min | Max |
| --- | ---: | ---: | ---: | ---: |
| `transactions-personal` | `21.85ms` | `0.76ms` | `20.85ms` | `23.04ms` |
| `cashflow-business` | `12.62ms` | `0.61ms` | `11.47ms` | `13.43ms` |
| `summary-report` | `34.71ms` | `0.63ms` | `33.67ms` | `35.49ms` |
| `summary-dashboard` | `72.83ms` | `1.41ms` | `70.54ms` | `75.08ms` |

Interpretation:

- The optimized Rust read path is materially faster than the original QA-001 baseline, especially for `summary-report` and `summary-dashboard`.
- These numbers are useful for end-to-end CLI comparison, but they are not the acceptance metric for the `sub-5 ms` API target.

## fin-api Warm Benchmark Snapshot

Warm handler latency from `meta.elapsed`:

| Endpoint | Mean ms | Min ms | Max ms | Runs |
| --- | ---: | ---: | ---: | ---: |
| `view.transactions?group=personal&limit=1000` | `4.05` | `4` | `5` | `20` |
| `report.summary?months=12` | `11.70` | `11` | `15` | `20` |
| `dashboard.kpis?group=business&months=24` | `4.00` | `3` | `5` | `20` |
| `dashboard.allocation?group=personal&month=2026-03` | `4.05` | `4` | `5` | `20` |
| `dashboard.hierarchy?group=business&months=6&mode=monthly_average` | `3.45` | `3` | `5` | `20` |
| `dashboard.flow?group=business&months=6&mode=monthly_average` | `4.15` | `4` | `5` | `20` |
| `dashboard.balances?account=Assets:Personal:Checking&downsampleMinStepDays=30` | `2.00` | `1` | `3` | `20` |
| `dashboard.contributions?account=Assets:Personal:Investments&downsampleMinStepDays=30` | `1.10` | `1` | `2` | `20` |
| `dashboard.projection?group=business&months=12` | `4.55` | `4` | `6` | `20` |

Warm end-to-end request timings from `hyperfine`:

| Endpoint | Mean ms |
| --- | ---: |
| `view.transactions` | `11.74` |
| `report.summary` | `16.89` |
| `dashboard.kpis` | `10.54` |
| `dashboard.allocation` | `10.59` |
| `dashboard.hierarchy` | `9.35` |
| `dashboard.flow` | `9.99` |
| `dashboard.balances` | `7.64` |
| `dashboard.contributions` | `6.82` |
| `dashboard.projection` | `9.55` |

Response sizes:

| Endpoint | Bytes |
| --- | ---: |
| `view.transactions` | `368228` |
| `report.summary` | `1344` |
| `dashboard.kpis` | `974` |
| `dashboard.allocation` | `1662` |
| `dashboard.hierarchy` | `514` |
| `dashboard.flow` | `2307` |
| `dashboard.balances` | `2482` |
| `dashboard.contributions` | `2609` |
| `dashboard.projection` | `2589` |

Interpretation:

- The roadmap's `sub-5 ms` target is met for the 1000-row transactions page when measured as warm API handler latency.
- Do not claim `sub-5 ms` for end-to-end transport or SSR. The `curl` path remains materially higher because it includes HTTP, Unix socket, JSON parsing, and client overhead.
- `report.summary` remains the heaviest warm API handler at `11.70 ms`, which is acceptable for the final roadmap state but should not be conflated with the lighter dashboard-specific endpoints.

## Manual Validation Results

### Web Offline State

- Ran the web server against `FIN_API_BASE_URL='http://127.0.0.1:9'`.
- Fetched `http://127.0.0.1:3000`.
- Confirmed SSR output rendered `API OFFLINE`.
- Confirmed the rendered detail text explained the failed backend URL rather than silently falling back.

### Dev Orchestration and Stale Socket Recovery

- Created a stale Unix socket at `target/bench-fixtures/benchmark-runtime/run/fin-api.sock`.
- Ran `FIN_HOME="$(pwd)/target/bench-fixtures/benchmark-runtime" bun run dev`.
- Confirmed `/v1/health` became reachable through the recreated socket.
- Confirmed the web header reached `API CONNECTED`.
- Sent `SIGINT` and confirmed the socket path was removed on shutdown.

### TUI Wide and Narrow Smoke

- Ran a wide 160x48 TUI session and exited cleanly.
- Ran a narrower 100x32 TUI session, switched to Transactions, paged, and exited cleanly.
- Confirmed both PTY captures contained terminal enter or exit sequences only and no panic text.

## Final Quality Gate

- `bun run util:check` passed after the final QA documentation updates.

## Residual Risks

- `report.summary` still has materially higher handler cost than the dashboard-specific endpoints.
- The final TUI validation is a real PTY smoke test, not a pixel-level visual diff or screenshot review.
- Unix socket transport is the default validated path. TCP fallback exists for portability, but the benchmark numbers above are for Unix sockets.

## Artifacts

- Final CLI warm timings: [`qa-002-cli-warm.json`](/Users/han/Git/fin/docs/benchmarks/generated/qa-002-cli-warm.json)
- Final cold timing for `transactions-personal`: [`qa-002-transactions-personal-cold.txt`](/Users/han/Git/fin/docs/benchmarks/generated/qa-002-transactions-personal-cold.txt)
- Final cold timing for `cashflow-business`: [`qa-002-cashflow-business-cold.txt`](/Users/han/Git/fin/docs/benchmarks/generated/qa-002-cashflow-business-cold.txt)
- Final cold timing for `summary-report`: [`qa-002-summary-report-cold.txt`](/Users/han/Git/fin/docs/benchmarks/generated/qa-002-summary-report-cold.txt)
- Final cold timing for `summary-dashboard`: [`qa-002-summary-dashboard-cold.txt`](/Users/han/Git/fin/docs/benchmarks/generated/qa-002-summary-dashboard-cold.txt)
- fin-api warm request timings: [`api-007-fin-api-warm.json`](/Users/han/Git/fin/docs/benchmarks/generated/api-007-fin-api-warm.json)
- fin-api handler timing samples: [`api-007-fin-api-handler-elapsed.csv`](/Users/han/Git/fin/docs/benchmarks/generated/api-007-fin-api-handler-elapsed.csv)
- fin-api response sizes: [`api-007-fin-api-response-sizes.csv`](/Users/han/Git/fin/docs/benchmarks/generated/api-007-fin-api-response-sizes.csv)
