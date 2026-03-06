# PERF-003 Transaction Path Benchmarks

This note records the validation for `PERF-003`.

## Scope

- Optimize the paginated transaction list path for large ledgers.
- Improve the search path without adding heavyweight storage unless the evidence requires it.
- Record whether the `sub-5 ms` target is met on the benchmark fixture.

## Changes

- Scoped transaction pages now treat `chart_account_ids` as exact chart-account IDs.
- Scoped transaction pages no longer join `chart_of_accounts` when the account scope is already known.
- Added one composite postings index:
  - `idx_postings_account_journal_entry_id ON postings(account_id, journal_entry_id, id)`
- Added an internal benchmark command:
  - `read_fixture transactions-page <home> <group> <limit> [search]`

## Query Plan Change

Before, the scoped list path planned as:

```text
SEARCH coa USING INDEX idx_chart_of_accounts_type
SEARCH p USING INDEX idx_postings_account
SEARCH je USING INDEX sqlite_autoindex_journal_entries_1
USE TEMP B-TREE FOR ORDER BY
```

After, the scoped list path plans as:

```text
SEARCH p USING INDEX idx_postings_account
SEARCH je USING INDEX sqlite_autoindex_journal_entries_1
USE TEMP B-TREE FOR ORDER BY
```

The search path keeps the correlated subquery for counterparty account matching, but it also drops the `chart_of_accounts` join on the scoped path.

## Correctness Guardrails

The ticket adds or updates tests for:

- pagination stability and sorting invariants
- search correctness
- explicit chart-account scoping returning only the requested account
- migration coverage for the new transaction query index

## Benchmark Fixtures

Use isolated fixtures so the baseline is not polluted by the new index migration:

```bash
CARGO_TARGET_DIR=/tmp/fin-perf-003-before/target cargo run -q --manifest-path /tmp/fin-perf-003-before/Cargo.toml -p fin-sdk --example build_fixture -- /tmp/fin-perf-003-before-runtime-x10 10
cargo run -q -p fin-sdk --example build_fixture -- target/bench-fixtures/perf-003-after-runtime-x10 10
```

## Benchmark Method

Measure the paginated transaction path in-process through `read_fixture` so the result reflects query and aggregation work instead of process startup.

Commands used:

```bash
hyperfine --shell=none --warmup 3 --runs 12 \
  --export-json docs/benchmarks/generated/perf-003-transactions-page-personal-x10.json \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=20 /tmp/fin-perf-003-before/target/release/examples/read_fixture transactions-page /tmp/fin-perf-003-before-runtime-x10 personal 5000' \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=20 /Users/han/Git/fin/target/release/examples/read_fixture transactions-page /Users/han/Git/fin/target/bench-fixtures/perf-003-after-runtime-x10 personal 5000'

hyperfine --shell=none --warmup 3 --runs 12 \
  --export-json docs/benchmarks/generated/perf-003-transactions-search-business-x10.json \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=20 /tmp/fin-perf-003-before/target/release/examples/read_fixture transactions-page /tmp/fin-perf-003-before-runtime-x10 business 1000 studio' \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=20 /Users/han/Git/fin/target/release/examples/read_fixture transactions-page /Users/han/Git/fin/target/bench-fixtures/perf-003-after-runtime-x10 business 1000 studio'

hyperfine --shell=none --warmup 3 --runs 10 \
  --export-json docs/benchmarks/generated/perf-003-transactions-page-personal-1000-current.json \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=40 /Users/han/Git/fin/target/release/examples/read_fixture transactions-page /Users/han/Git/fin/target/bench-fixtures/perf-003-after-runtime-x10 personal 1000'
```

## Results

Results are stored in:

- `docs/benchmarks/generated/perf-003-transactions-page-personal-x10.json`
- `docs/benchmarks/generated/perf-003-transactions-search-business-x10.json`
- `docs/benchmarks/generated/perf-003-transactions-page-personal-1000-current.json`

Summary:

| Target | Before mean | After mean | Improvement |
|---|---:|---:|---:|
| `transactions-page personal 5000`, `20` iterations | `521.3ms` | `406.9ms` | `1.28x` faster |
| `transactions-page business 1000 studio`, `20` iterations | `678.7ms` | `572.3ms` | `1.19x` faster |

Current-only budget check:

| Target | Mean | Approx per query |
|---|---:|---:|
| `transactions-page personal 1000`, `40` iterations | `441.5ms` | `11.0ms` |

## Interpretation

- The paginated list path improved materially with a small reversible change set.
- The search path also improved without introducing FTS or a separate search projection.
- The `sub-5 ms` target is still not met on the x10 fixture for a 1,000-row page. The measured warm path is about `11.0ms` per query.

## Exception

This ticket intentionally stops short of FTS or a dedicated transaction projection table.

Reason:

- the current changes already improve both list and search paths
- the remaining gap to `sub-5 ms` would require a larger storage design change
- that change is not minimal, and it would add ongoing maintenance cost for imports and future mutations

The final roadmap should treat the current transaction path as improved but still above the aspirational budget on the synthetic x10 fixture.
