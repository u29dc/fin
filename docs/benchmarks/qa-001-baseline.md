# QA-001 Baseline

This document records the initial benchmark baseline for the synthetic fixture introduced in `QA-001`.

## Fixture

- Source dir: `tests/fixtures/benchmark`
- Generated runtime: `target/bench-fixtures/benchmark-runtime`
- Generator: `cargo run -q -p fin-sdk --example build_fixture -- target/bench-fixtures/benchmark-runtime`
- Seed: `qa-001-benchmark-fixture`
- Months: `48`
- Accounts: `8`
- Journal entries: `3,229`
- Postings: `6,458`
- Transfer entries: `240`
- Date range: `2022-12-02T08:15:00` -> `2026-12-28T15:20:00`

## Commands

Cold reference commands:

```bash
/usr/bin/time -lp target/debug/fin --json view transactions --group personal --limit 1000 >/dev/null
/usr/bin/time -lp target/debug/fin --json report cashflow --group business --months 24 >/dev/null
/usr/bin/time -lp target/debug/fin --json report summary >/dev/null
/usr/bin/time -lp target/debug/examples/read_fixture summary-dashboard target/bench-fixtures/benchmark-runtime >/dev/null
```

Warm benchmark command:

```bash
scripts/benchmarks/baseline.sh
```

## Baseline Results

Cold one-shot timings from `docs/benchmarks/generated/*-cold.txt`:

| Target | Real | User | Sys |
|---|---:|---:|---:|
| `transactions-personal` | `0.05s` | `0.01s` | `0.01s` |
| `cashflow-business` | `0.01s` | `0.01s` | `0.00s` |
| `summary-report` | `0.07s` | `0.06s` | `0.00s` |
| `summary-dashboard` | `0.55s` | `0.13s` | `0.01s` |

Warm repeated timings from `docs/benchmarks/generated/warm.json`:

| Target | Mean | Std Dev | Min | Max |
|---|---:|---:|---:|---:|
| `transactions-personal` | `20.9ms` | `1.9ms` | `17.7ms` | `23.8ms` |
| `cashflow-business` | `13.2ms` | `2.4ms` | `11.1ms` | `18.4ms` |
| `summary-report` | `73.0ms` | `3.4ms` | `67.1ms` | `79.2ms` |
| `summary-dashboard` | `106.2ms` | `2.6ms` | `103.3ms` | `111.2ms` |

## Interpretation

- The current read path is already acceptable for basic cashflow and medium transaction pages on the synthetic fixture.
- `summary-report` and the assembled `summary-dashboard` are materially slower than the other baseline targets and should improve in the later SDK and performance tickets.
- The roadmap's final `sub-5 ms` target is not met by the current transaction path; this baseline exists so `PERF-003` can prove a real improvement instead of relying on intuition.
