# PERF-002 Shared Series Benchmarks

This note records the validation for `PERF-002`.

## Scope

- Remove repeated `group_monthly_cashflow` calls from `report_summary`.
- Reuse one per-group cashflow context and derive summary metrics from it.
- Keep standalone `report_cashflow`, `report_runway`, and `report_reserves` on their existing cost profile.

## Refactor Shape

Before:

- `report_summary` called `report_runway`, `report_health`, `report_reserves`, and `report_cashflow_kpis` for every group.
- That triggered `group_monthly_cashflow` `4` times per group.
- `report_cashflow_kpis` also queried `current_reporting_month` once per group.

After:

- `report_summary` builds one `GroupReportContext` per group.
- The summary route computes latest runway, health, and reserves in one pass over that shared context.
- `current_reporting_month` is queried once per summary call instead of once per group.
- Standalone `report_runway` and `report_reserves` derive only the series they need from the shared context, so they do not allocate unrelated report vectors.

## Correctness Guardrails

The ticket adds fixture-backed tests in `crates/fin-sdk/src/reports.rs` for:

- `report_cashflow` totals matching the returned series
- `report_summary` matching the latest values from the individual report functions
- runway fallback preserving current-balance behavior when the filtered history is empty

## Benchmark Fixture

- Fixture root: `target/bench-fixtures/benchmark-runtime-x10`
- Generator command:

```bash
cargo run -q -p fin-sdk --example build_fixture -- target/bench-fixtures/benchmark-runtime-x10 10
```

## Benchmark Method

Measure the report functions in-process through `crates/fin-sdk/examples/read_fixture.rs` so the numbers mostly reflect query and aggregation work instead of process startup.

Commands used:

```bash
cargo build -q -p fin-sdk --example read_fixture --release
CARGO_TARGET_DIR=/tmp/fin-perf-001-before/target cargo build -q --manifest-path /tmp/fin-perf-001-before/Cargo.toml -p fin-sdk --example read_fixture --release

hyperfine --shell=none --warmup 3 --runs 15 \
  --export-json docs/benchmarks/generated/perf-002-summary-report-x10.json \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=10 /tmp/fin-perf-001-before/target/release/examples/read_fixture summary-report /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10' \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=10 /Users/han/Git/fin/target/release/examples/read_fixture summary-report /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10'

hyperfine --shell=none --warmup 3 --runs 15 \
  --export-json docs/benchmarks/generated/perf-002-cashflow-business-x10.json \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=80 /tmp/fin-perf-001-before/target/release/examples/read_fixture cashflow /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10 business 24' \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=80 /Users/han/Git/fin/target/release/examples/read_fixture cashflow /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10 business 24'

hyperfine --shell=none --warmup 3 --runs 15 \
  --export-json docs/benchmarks/generated/perf-002-runway-business-x10.json \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=60 /tmp/fin-perf-001-before/target/release/examples/read_fixture runway /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10 business' \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=60 /Users/han/Git/fin/target/release/examples/read_fixture runway /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10 business'

hyperfine --shell=none --warmup 3 --runs 15 \
  --export-json docs/benchmarks/generated/perf-002-reserves-business-x10.json \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=60 /tmp/fin-perf-001-before/target/release/examples/read_fixture reserves /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10 business' \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=60 /Users/han/Git/fin/target/release/examples/read_fixture reserves /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10 business'
```

## Results

Results are stored in:

- `docs/benchmarks/generated/perf-002-summary-report-x10.json`
- `docs/benchmarks/generated/perf-002-cashflow-business-x10.json`
- `docs/benchmarks/generated/perf-002-runway-business-x10.json`
- `docs/benchmarks/generated/perf-002-reserves-business-x10.json`

Summary:

| Target | Before mean | After mean | Interpretation |
|---|---:|---:|---|
| `summary-report`, `10` iterations | `3.616s` | `1.074s` | `3.37x` faster |
| `cashflow business 24`, `80` iterations | `2.288s` | `2.250s` | effectively flat, slight improvement |
| `runway business`, `60` iterations | `1.742s` | `1.746s` | flat within noise |
| `reserves business`, `60` iterations | `1.739s` | `1.747s` | flat within noise |

## Interpretation

- `report_summary` now avoids the main repeated-scan problem and shows a material warm-path improvement on the benchmark fixture.
- `report_cashflow` was already close to its minimum structure, so only a small change was expected.
- `report_runway` and `report_reserves` remain effectively flat after splitting the shared context from the targeted derivations. They no longer pay for unrelated report vectors.
