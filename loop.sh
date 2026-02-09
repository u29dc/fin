#!/usr/bin/env bash
set -euo pipefail

# Autonomous Agent Loop Harness
# Usage: ./loop.sh [max_iterations] [--model MODEL] [--no-push]

PROMPT_FILE="PROMPT.md"
LOG_DIR="agent_logs"
MAX_ITERATIONS=0
MODEL=""
AUTO_PUSH=true
BACKOFF_BASE=5
BACKOFF_MAX=60
CONSECUTIVE_FAILURES=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --model) MODEL="$2"; shift 2 ;;
    --no-push) AUTO_PUSH=false; shift ;;
    [0-9]*) MAX_ITERATIONS="$1"; shift ;;
    *) echo "Unknown argument: $1"; exit 1 ;;
  esac
done

# --- Prereq checks ---
if [[ ! -f "$PROMPT_FILE" ]]; then
  echo "Error: $PROMPT_FILE not found"; exit 1
fi
if [[ ! -s "$PROMPT_FILE" ]]; then
  echo "Error: $PROMPT_FILE is empty"; exit 1
fi
if ! command -v claude &>/dev/null; then
  echo "Error: claude not found in PATH"; exit 1
fi
if ! git rev-parse --is-inside-work-tree &>/dev/null; then
  echo "Error: not inside a git repository"; exit 1
fi

mkdir -p "$LOG_DIR"
ITERATION=0
CURRENT_BRANCH=$(git branch --show-current)
if [[ -z "$CURRENT_BRANCH" ]]; then
  echo "Error: detached HEAD -- checkout a branch first"; exit 1
fi
START_TIME=$(date +%s)

# --- Signal handling ---
cleanup() {
  echo ""
  echo "Loop interrupted after $ITERATION iterations"
  echo "Duration: $(( $(date +%s) - START_TIME )) seconds"
  echo "Branch: $CURRENT_BRANCH"
  echo "Logs: $LOG_DIR/"
  exit 0
}
trap cleanup SIGINT SIGTERM

# --- Auto-push safety ---
try_push() {
  if [[ "$AUTO_PUSH" != true ]]; then return; fi
  if ! git remote get-url origin &>/dev/null; then return; fi
  local ahead
  ahead=$(git rev-list --count "origin/$CURRENT_BRANCH..$CURRENT_BRANCH" 2>/dev/null || echo "0")
  if [[ "$ahead" == "0" ]]; then return; fi
  git push origin "$CURRENT_BRANCH" 2>/dev/null || {
    echo "Push failed. Will retry next iteration."
  }
}

echo "=========================================="
echo " Agent Loop Starting"
echo " Branch: $CURRENT_BRANCH"
echo " Model: ${MODEL:-default}"
echo " Max iterations: ${MAX_ITERATIONS:-unlimited}"
echo " Auto-push: $AUTO_PUSH"
echo "=========================================="

# --- Main loop ---
while true; do
  if [[ $MAX_ITERATIONS -gt 0 ]] && [[ $ITERATION -ge $MAX_ITERATIONS ]]; then
    echo "Reached max iterations: $MAX_ITERATIONS"
    break
  fi

  ITERATION=$((ITERATION + 1))
  TIMESTAMP=$(date +%Y%m%d_%H%M%S)
  COMMIT_HASH=$(git rev-parse --short HEAD 2>/dev/null || echo "no-commit")
  LOG_FILE="$LOG_DIR/agent_${TIMESTAMP}_${COMMIT_HASH}.log"

  echo ""
  echo "================ ITERATION $ITERATION ================"
  echo "Time: $(date '+%Y-%m-%d %H:%M:%S')"
  echo "Log: $LOG_FILE"

  ITER_START=$(date +%s)
  set +e
  claude -p "$(cat "$PROMPT_FILE")" \
    --dangerously-skip-permissions \
    ${MODEL:+--model "$MODEL"} \
    --verbose \
    2>&1 | tee "$LOG_FILE"
  EXIT_CODE=${PIPESTATUS[0]}
  set -e

  ITER_DURATION=$(( $(date +%s) - ITER_START ))
  echo "Duration: ${ITER_DURATION}s | Exit: $EXIT_CODE" >> "$LOG_FILE"

  # Check for completion
  if grep -q '<promise>COMPLETE</promise>' "$LOG_FILE" 2>/dev/null; then
    echo ""
    echo "=========================================="
    echo " ALL STORIES COMPLETE"
    echo " Iterations: $ITERATION"
    echo " Total duration: $(( $(date +%s) - START_TIME ))s"
    echo "=========================================="
    try_push
    break
  fi

  # Handle failures with backoff
  if [[ $EXIT_CODE -ne 0 ]]; then
    CONSECUTIVE_FAILURES=$((CONSECUTIVE_FAILURES + 1))
    BACKOFF=$(( BACKOFF_BASE * (2 ** (CONSECUTIVE_FAILURES - 1)) ))
    if [[ $BACKOFF -gt $BACKOFF_MAX ]]; then
      BACKOFF=$BACKOFF_MAX
    fi
    echo "Iteration failed (exit $EXIT_CODE). Backoff: ${BACKOFF}s (failure #$CONSECUTIVE_FAILURES)"
    sleep "$BACKOFF"
  else
    CONSECUTIVE_FAILURES=0
    try_push
  fi
done
