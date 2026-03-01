#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

blocked_paths=(
  "packages"
  "scripts/parity"
)

for path in "${blocked_paths[@]}"; do
  if [[ -e "$path" ]]; then
    echo "legacy guard failed: path must not exist: $path" >&2
    exit 1
  fi
done

declare -a scan_targets=(
  "package.json"
  "scripts"
  "crates"
  "AGENTS.md"
  ".claude/skills/fin/SKILL.md"
  "docs"
)

declare -a blocked_patterns=(
  "scripts/parity"
  "certify:parity"
  "bun run packages/cli/src/index.ts"
)

for pattern in "${blocked_patterns[@]}"; do
  if rg -n --hidden --glob '!.git/**' --glob '!scripts/rust/guard_no_legacy.sh' "$pattern" "${scan_targets[@]}" >/dev/null 2>&1; then
    echo "legacy guard failed: found blocked pattern '$pattern'" >&2
    rg -n --hidden --glob '!.git/**' --glob '!scripts/rust/guard_no_legacy.sh' "$pattern" "${scan_targets[@]}" >&2
    exit 1
  fi
done

echo "legacy guard passed"
