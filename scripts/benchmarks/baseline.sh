#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FIXTURE_HOME="${1:-$ROOT/target/bench-fixtures/benchmark-runtime}"
OUT_DIR="${2:-$ROOT/docs/benchmarks/generated}"
CLI_BIN="$ROOT/target/debug/fin"
SDK_EXAMPLE_BIN="$ROOT/target/debug/examples/read_fixture"

mkdir -p "$OUT_DIR"

cargo run -q -p fin-sdk --example build_fixture -- "$FIXTURE_HOME" > "$OUT_DIR/fixture-materialization.json"
cargo build -q -p fin-cli -p fin-sdk --examples

export FIN_HOME="$FIXTURE_HOME"

run_cold() {
  local name="$1"
  local command="$2"
  local output="$OUT_DIR/$name-cold.txt"
  /usr/bin/time -lp bash -lc "$command >/dev/null" > /dev/null 2> "$output"
}

run_cold "transactions-personal" "$CLI_BIN view transactions --group personal --limit 1000"
run_cold "cashflow-business" "$CLI_BIN report cashflow --group business --months 24"
run_cold "summary-report" "$CLI_BIN report summary"
run_cold "summary-dashboard" "$SDK_EXAMPLE_BIN summary-dashboard $FIXTURE_HOME"

hyperfine \
  --warmup 3 \
  --runs 10 \
  --export-json "$OUT_DIR/warm.json" \
  --command-name transactions-personal "$CLI_BIN view transactions --group personal --limit 1000 >/dev/null" \
  --command-name cashflow-business "$CLI_BIN report cashflow --group business --months 24 >/dev/null" \
  --command-name summary-report "$CLI_BIN report summary >/dev/null" \
  --command-name summary-dashboard "$SDK_EXAMPLE_BIN summary-dashboard $FIXTURE_HOME >/dev/null"
