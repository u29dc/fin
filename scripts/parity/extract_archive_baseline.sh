#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
out_dir="$repo_root/scripts/parity/archive-baseline"

find "$out_dir" -mindepth 1 -delete 2>/dev/null || true
mkdir -p "$out_dir/source"

archive_sha="$(git -C "$repo_root" rev-parse archive)"

cat > "$out_dir/metadata.txt" <<META
archive_branch=archive
archive_sha=$archive_sha
generated_utc=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
META

# Source snapshots for critical command contracts
key_files=(
  "packages/cli/src/envelope.ts"
  "packages/cli/src/tool.ts"
  "packages/cli/src/main.ts"
  "packages/cli/src/commands/health/index.ts"
  "packages/cli/src/commands/tools/index.ts"
  "packages/cli/src/commands/config/show.ts"
  "packages/cli/src/commands/config/validate.ts"
  "packages/core/src/config/loader.ts"
  "packages/core/src/db/schema.ts"
  "packages/core/src/db/migrate.ts"
)

for file in "${key_files[@]}"; do
  mkdir -p "$out_dir/source/$(dirname "$file")"
  git -C "$repo_root" show "archive:$file" > "$out_dir/source/$file.snapshot.txt"
done

# Capture command names declared in command modules
# This intentionally captures both tool names (e.g. report.summary)
# and top-level command names (e.g. report, view, config) for inventory.
git -C "$repo_root" grep -h "name: '" archive -- packages/cli/src/commands \
  | sed -E "s/.*name: '([^']+)'.*/\\1/" \
  | rg "^[a-z]+(\.[a-z]+)?$" \
  | sort -u > "$out_dir/command-names.txt"

# Snapshot command file list for drift checks
git -C "$repo_root" ls-tree -r --name-only archive -- packages/cli/src/commands \
  | sort > "$out_dir/command-files.txt"

# Snapshot core query/import/sanitize source files for parity tracing
git -C "$repo_root" ls-tree -r --name-only archive -- packages/core/src \
  | rg "^(packages/core/src/(queries|import|sanitize)/.*\\.ts)$" \
  | sort > "$out_dir/core-domain-files.txt"

cat > "$out_dir/README.md" <<'README'
# Archive Baseline

This directory is generated from the immutable `archive` branch and is used
as a source-contract baseline for parity checks during the Rust rewrite.

Contents:

- `metadata.txt`: archive SHA and generation timestamp
- `source/**.snapshot.txt`: snapshots of high-impact contract files
- `command-names.txt`: discovered command names from archive command modules
- `command-files.txt`: command module file inventory
- `core-domain-files.txt`: core import/sanitize/query file inventory

Regenerate with:

```bash
./scripts/parity/extract_archive_baseline.sh
```
README

echo "Archive baseline written to: $out_dir"
