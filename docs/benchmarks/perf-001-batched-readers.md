# PERF-001 Batched Reader Benchmarks

This note records the validation for `PERF-001`.

## Scope

- `view_accounts`: replace per-account balance and freshness queries with one grouped aggregate query.
- `view_ledger`: replace one postings query per journal entry with one batched postings query for the whole result set.

## Correctness Guardrails

- Query result ordering must not change.
- Payload shape must not change.
- Empty filters must still return empty results without invalid SQL.

The ticket adds fixture-backed tests in `crates/fin-sdk/src/queries.rs` for:

- config-order preservation in `view_accounts`
- journal entry and posting ordering in `view_ledger`
- filtered ledger correctness when an account filter is applied

## Query Count Change

- `view_accounts` before: `2 * asset_account_count` queries after the config filter.
- `view_accounts` after: `1` grouped aggregate query for all filtered asset accounts.
- `view_ledger` before: `1 + entry_count` queries for the selected page.
- `view_ledger` after: `2` queries total: one for entries, one for all postings in that page.

## Benchmark Fixture

- Fixture root: `target/bench-fixtures/benchmark-runtime-x10`
- Generator command:

```bash
cargo run -q -p fin-sdk --example build_fixture -- target/bench-fixtures/benchmark-runtime-x10 10
```

- Dataset shape:
  - same deterministic synthetic fixture from `QA-001`
  - transaction volume scaled by `10x`

## Benchmark Method

CLI timings on the standard fixture remained mostly flat because process startup dominated the measured time.

For this ticket, measure the query path in-process:

- build the shared benchmark example on the pre-change worktree and the current worktree
- open the fixture once per benchmark process
- repeat the target read many times in the same process via `READ_FIXTURE_ITERATIONS`

Commands used:

```bash
CARGO_TARGET_DIR=/tmp/fin-perf-001-before/target cargo build -q --manifest-path /tmp/fin-perf-001-before/Cargo.toml -p fin-sdk --example read_fixture --release
cargo build -q -p fin-sdk --example read_fixture --release

hyperfine --shell=none --warmup 3 --runs 20 \
  --export-json docs/benchmarks/generated/perf-001-sdk-accounts.json \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=200 /tmp/fin-perf-001-before/target/release/examples/read_fixture accounts /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10 personal' \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=200 /Users/han/Git/fin/target/release/examples/read_fixture accounts /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10 personal'

hyperfine --shell=none --warmup 3 --runs 15 \
  --export-json docs/benchmarks/generated/perf-001-sdk-ledger-x10.json \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=20 /tmp/fin-perf-001-before/target/release/examples/read_fixture ledger /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10 5000' \
  '/usr/bin/env READ_FIXTURE_ITERATIONS=20 /Users/han/Git/fin/target/release/examples/read_fixture ledger /Users/han/Git/fin/target/bench-fixtures/benchmark-runtime-x10 5000'
```

## Results

Results are stored in:

- `docs/benchmarks/generated/perf-001-sdk-accounts.json`
- `docs/benchmarks/generated/perf-001-sdk-ledger-x10.json`

Summary:

| Target | Before mean | After mean | Improvement |
|---|---:|---:|---:|
| `accounts` on `personal`, `200` iterations | `686.8ms` | `513.7ms` | `1.34x` faster |
| `ledger`, `limit 5000`, `20` iterations | `612.5ms` | `296.9ms` | `2.06x` faster |

## Interpretation

- The grouped account aggregate removes enough repeated SQL to produce a clear warm-path win.
- The batched ledger postings load is materially faster once startup noise is removed from the measurement.
- The fixture-backed tests confirm the refactor did not change ordering or account-filter semantics.
