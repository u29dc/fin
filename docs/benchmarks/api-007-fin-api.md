# API-007 fin-api Benchmarks

## Scope

Recorded on 2026-03-06 against the committed synthetic fixture.

- Fixture command: `cargo run -q -p fin-sdk --example build_fixture -- target/bench-fixtures/api-runtime`
- Benchmark command: `bun run util:bench:api`
- Binary mode: `target/release/fin-api`
- Transport: Unix domain socket
- Handler timing source: `meta.elapsed` sampled over 20 requests
- End-to-end timing source: `hyperfine` around `curl --unix-socket ...`

`meta.elapsed` is the right number for the sub-5 ms transaction target.
`hyperfine` includes client, HTTP parsing, Unix socket, and `curl` overhead.

## Warm Handler Latency

| Endpoint | Mean ms | Min ms | Max ms | Runs |
| --- | ---: | ---: | ---: | ---: |
| `view.transactions?group=personal&limit=1000` | 4.20 | 3 | 8 | 20 |
| `report.summary?months=12` | 11.00 | 10 | 12 | 20 |
| `dashboard.kpis?group=business&months=24` | 3.50 | 3 | 4 | 20 |
| `dashboard.allocation?group=personal&month=2026-03` | 3.90 | 3 | 5 | 20 |
| `dashboard.hierarchy?group=business&months=6&mode=monthly_average` | 3.05 | 3 | 4 | 20 |
| `dashboard.flow?group=business&months=6&mode=monthly_average` | 3.95 | 3 | 4 | 20 |
| `dashboard.balances?account=Assets:Personal:Checking&downsampleMinStepDays=30` | 1.30 | 1 | 2 | 20 |
| `dashboard.contributions?account=Assets:Personal:Investments&downsampleMinStepDays=30` | 1.05 | 1 | 2 | 20 |
| `dashboard.projection?group=business&months=12` | 4.05 | 4 | 5 | 20 |

Result:

- The 1000-row transactions page meets the sub-5 ms warm handler target on the fixture.
- Summary aggregation remains materially heavier than the dashboard-specific endpoints.

## End-To-End Warm Request Latency

| Endpoint | Mean ms |
| --- | ---: |
| `view.transactions` | 11.53 |
| `report.summary` | 18.94 |
| `dashboard.kpis` | 14.92 |
| `dashboard.allocation` | 11.95 |
| `dashboard.hierarchy` | 11.41 |
| `dashboard.flow` | 9.97 |
| `dashboard.balances` | 8.69 |
| `dashboard.contributions` | 7.55 |
| `dashboard.projection` | 12.28 |

Interpretation:

- End-to-end local request time is consistently higher than handler time because the measurement includes `curl` plus HTTP plus Unix socket overhead.
- The dashboard endpoints stay materially lighter than `report.summary`.

## Response Sizes

| Endpoint | Bytes |
| --- | ---: |
| `view.transactions` | 368228 |
| `report.summary` | 1344 |
| `dashboard.kpis` | 974 |
| `dashboard.allocation` | 1662 |
| `dashboard.hierarchy` | 514 |
| `dashboard.flow` | 2307 |
| `dashboard.balances` | 2482 |
| `dashboard.contributions` | 2609 |
| `dashboard.projection` | 2589 |

Notes:

- The transactions page is the only intentionally large payload in this set.
- Dashboard endpoints remain small enough for repeated local SSR reads without over-fetching.

## Artifacts

- Warm request timings: [`api-007-fin-api-warm.json`](/Users/han/Git/fin/docs/benchmarks/generated/api-007-fin-api-warm.json)
- Handler timing samples: [`api-007-fin-api-handler-elapsed.csv`](/Users/han/Git/fin/docs/benchmarks/generated/api-007-fin-api-handler-elapsed.csv)
- Response sizes: [`api-007-fin-api-response-sizes.csv`](/Users/han/Git/fin/docs/benchmarks/generated/api-007-fin-api-response-sizes.csv)
- Fixture materialization record: [`api-007-fin-api-fixture.json`](/Users/han/Git/fin/docs/benchmarks/generated/api-007-fin-api-fixture.json)
