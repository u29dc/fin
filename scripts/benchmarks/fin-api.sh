#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FIXTURE_HOME="${1:-$ROOT/target/bench-fixtures/api-runtime}"
OUT_DIR="${2:-$ROOT/docs/benchmarks/generated}"
SOCKET_PATH="${3:-$FIXTURE_HOME/run/fin-api.sock}"
API_BIN="$ROOT/target/release/fin-api"
FIXTURE_BIN_ARGS=(-q -p fin-sdk --example build_fixture -- "$FIXTURE_HOME")
CONFIG_PATH="$FIXTURE_HOME/data/fin.config.toml"
DB_PATH="$FIXTURE_HOME/data/fin.db"
SIZES_CSV="$OUT_DIR/api-007-fin-api-response-sizes.csv"
WARM_JSON="$OUT_DIR/api-007-fin-api-warm.json"
HANDLER_CSV="$OUT_DIR/api-007-fin-api-handler-elapsed.csv"
SERVER_LOG="$OUT_DIR/api-007-fin-api-server.log"

mkdir -p "$OUT_DIR"

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "hyperfine is required for fin-api benchmarks" >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required for fin-api benchmarks" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required for fin-api benchmarks" >&2
  exit 1
fi

cargo run "${FIXTURE_BIN_ARGS[@]}" > "$OUT_DIR/api-007-fin-api-fixture.json"
cargo build -q --release -p fin-api

cleanup() {
  if [[ -n "${API_PID:-}" ]]; then
    kill "$API_PID" >/dev/null 2>&1 || true
    wait "$API_PID" 2>/dev/null || true
  fi
  rm -f "$SOCKET_PATH"
}

trap cleanup EXIT

"$API_BIN" start \
  --socket-path "$SOCKET_PATH" \
  --config-path "$CONFIG_PATH" \
  --db-path "$DB_PATH" \
  > /dev/null 2> "$SERVER_LOG" &
API_PID=$!

for _ in $(seq 1 100); do
  if curl --silent --fail --output /dev/null --unix-socket "$SOCKET_PATH" "http://localhost/__probe"; then
    break
  fi
  sleep 0.1
done

if ! curl --silent --fail --output /dev/null --unix-socket "$SOCKET_PATH" "http://localhost/__probe"; then
  echo "fin-api did not become ready; see $SERVER_LOG" >&2
  exit 1
fi

capture_size() {
  local name="$1"
  local path="$2"
  local output
  output="$(mktemp "$OUT_DIR/$name.XXXXXX.json")"
  curl --silent --fail --unix-socket "$SOCKET_PATH" "http://localhost$path" > "$output"
  printf '%s,%s\n' "$name" "$(wc -c < "$output" | tr -d ' ')" >> "$SIZES_CSV"
  rm -f "$output"
}

cat > "$SIZES_CSV" <<'EOF'
endpoint,bytes
EOF

capture_size "api-007-transactions-page" "/v1/view/transactions?group=personal&limit=1000"
capture_size "api-007-report-summary" "/v1/report/summary?months=12"
capture_size "api-007-dashboard-kpis" "/v1/dashboard/kpis?group=business&months=24"
capture_size "api-007-dashboard-allocation" "/v1/dashboard/allocation?group=personal&month=2026-03"
capture_size "api-007-dashboard-hierarchy" "/v1/dashboard/hierarchy?group=business&months=6&mode=monthly_average"
capture_size "api-007-dashboard-flow" "/v1/dashboard/flow?group=business&months=6&mode=monthly_average"
capture_size "api-007-dashboard-balances" "/v1/dashboard/balances?account=Assets%3APersonal%3AChecking&downsampleMinStepDays=30"
capture_size "api-007-dashboard-contributions" "/v1/dashboard/contributions?account=Assets%3APersonal%3AInvestments&downsampleMinStepDays=30"
capture_size "api-007-dashboard-projection" "/v1/dashboard/projection?group=business&months=12"

sample_handler_elapsed() {
  local name="$1"
  local path="$2"
  local sample_file
  sample_file="$(mktemp "$OUT_DIR/$name-handler-samples.XXXXXX.txt")"
  : > "$sample_file"
  for _ in $(seq 1 20); do
    curl --silent --fail --unix-socket "$SOCKET_PATH" "http://localhost$path" \
      | python3 -c 'import json, sys; print(json.load(sys.stdin)["meta"]["elapsed"])' \
      >> "$sample_file"
  done
  python3 - "$name" "$sample_file" <<'PY' >> "$HANDLER_CSV"
import pathlib
import statistics
import sys

name = sys.argv[1]
sample_file = pathlib.Path(sys.argv[2])
samples = [int(line.strip()) for line in sample_file.read_text().splitlines() if line.strip()]
mean_value = statistics.mean(samples) if samples else 0.0
print(f"{name},{mean_value:.2f},{min(samples) if samples else 0},{max(samples) if samples else 0},{len(samples)}")
PY
  rm -f "$sample_file"
}

cat > "$HANDLER_CSV" <<'EOF'
endpoint,mean_ms,min_ms,max_ms,runs
EOF

sample_handler_elapsed "view.transactions" "/v1/view/transactions?group=personal&limit=1000"
sample_handler_elapsed "report.summary" "/v1/report/summary?months=12"
sample_handler_elapsed "dashboard.kpis" "/v1/dashboard/kpis?group=business&months=24"
sample_handler_elapsed "dashboard.allocation" "/v1/dashboard/allocation?group=personal&month=2026-03"
sample_handler_elapsed "dashboard.hierarchy" "/v1/dashboard/hierarchy?group=business&months=6&mode=monthly_average"
sample_handler_elapsed "dashboard.flow" "/v1/dashboard/flow?group=business&months=6&mode=monthly_average"
sample_handler_elapsed "dashboard.balances" "/v1/dashboard/balances?account=Assets%3APersonal%3AChecking&downsampleMinStepDays=30"
sample_handler_elapsed "dashboard.contributions" "/v1/dashboard/contributions?account=Assets%3APersonal%3AInvestments&downsampleMinStepDays=30"
sample_handler_elapsed "dashboard.projection" "/v1/dashboard/projection?group=business&months=12"

hyperfine \
  --warmup 5 \
  --runs 20 \
  --export-json "$WARM_JSON" \
  --command-name view.transactions "curl --silent --fail --output /dev/null --unix-socket '$SOCKET_PATH' 'http://localhost/v1/view/transactions?group=personal&limit=1000'" \
  --command-name report.summary "curl --silent --fail --output /dev/null --unix-socket '$SOCKET_PATH' 'http://localhost/v1/report/summary?months=12'" \
  --command-name dashboard.kpis "curl --silent --fail --output /dev/null --unix-socket '$SOCKET_PATH' 'http://localhost/v1/dashboard/kpis?group=business&months=24'" \
  --command-name dashboard.allocation "curl --silent --fail --output /dev/null --unix-socket '$SOCKET_PATH' 'http://localhost/v1/dashboard/allocation?group=personal&month=2026-03'" \
  --command-name dashboard.hierarchy "curl --silent --fail --output /dev/null --unix-socket '$SOCKET_PATH' 'http://localhost/v1/dashboard/hierarchy?group=business&months=6&mode=monthly_average'" \
  --command-name dashboard.flow "curl --silent --fail --output /dev/null --unix-socket '$SOCKET_PATH' 'http://localhost/v1/dashboard/flow?group=business&months=6&mode=monthly_average'" \
  --command-name dashboard.balances "curl --silent --fail --output /dev/null --unix-socket '$SOCKET_PATH' 'http://localhost/v1/dashboard/balances?account=Assets%3APersonal%3AChecking&downsampleMinStepDays=30'" \
  --command-name dashboard.contributions "curl --silent --fail --output /dev/null --unix-socket '$SOCKET_PATH' 'http://localhost/v1/dashboard/contributions?account=Assets%3APersonal%3AInvestments&downsampleMinStepDays=30'" \
  --command-name dashboard.projection "curl --silent --fail --output /dev/null --unix-socket '$SOCKET_PATH' 'http://localhost/v1/dashboard/projection?group=business&months=12'"
