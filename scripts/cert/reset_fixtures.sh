#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fin_home="${CERT_FIN_HOME:-$repo_root/.tmp/cert-fin-home}"
case_name="${1:-valid}"

data_dir="$fin_home/data"
inbox_dir="$fin_home/imports/inbox"
archive_dir="$fin_home/imports/archive"

mkdir -p "$data_dir" "$inbox_dir" "$archive_dir"

# Reset runtime state for deterministic tests
find "$inbox_dir" -type f -delete 2>/dev/null || true
find "$archive_dir" -type f -delete 2>/dev/null || true
find "$archive_dir" -type d -mindepth 1 -empty -delete 2>/dev/null || true

# Always reset DB to ensure deterministic fixture runs unless explicitly preserved
if [[ "${RESET_DB:-1}" == "1" ]]; then
  rm -f "$data_dir/fin.db"
fi

cp "$repo_root/fin.config.template.toml" "$data_dir/fin.config.toml"
# Provide a local minimal TS rules file for migrate-ts certification.
cat > "$data_dir/fin.rules.ts" <<'RULES_TS'
export const NAME_MAPPING_CONFIG = {
  rules: [],
  warnOnUnmapped: true,
  fallbackToRaw: true,
};
RULES_TS

# Also provision TOML sample for upcoming Rust rules migration.
cat > "$data_dir/fin.rules.toml" <<'RULES_TOML'
warn_on_unmapped = true
fallback_to_raw = true

[[rules]]
match = "example"
replace = "Example"
category = "Expenses:Uncategorized"
RULES_TOML

folders=(
  "business-wise"
  "business-monzo"
  "joint-monzo"
  "personal-investments"
  "personal-monzo"
  "personal-savings"
)

for folder in "${folders[@]}"; do
  mkdir -p "$inbox_dir/$folder"
  find "$inbox_dir/$folder" -type f -delete 2>/dev/null || true
done

copy_valid_set() {
  cp "$repo_root/scripts/cert/fixtures/business-wise.csv" "$inbox_dir/business-wise/wise-business.csv"
  cp "$repo_root/scripts/cert/fixtures/business-monzo.csv" "$inbox_dir/business-monzo/monzo-business.csv"
  cp "$repo_root/scripts/cert/fixtures/joint-monzo.csv" "$inbox_dir/joint-monzo/monzo-joint.csv"
  cp "$repo_root/scripts/cert/fixtures/personal-investments.csv" "$inbox_dir/personal-investments/vanguard.csv"
  cp "$repo_root/scripts/cert/fixtures/personal-monzo.csv" "$inbox_dir/personal-monzo/monzo-personal.csv"
  cp "$repo_root/scripts/cert/fixtures/personal-savings.csv" "$inbox_dir/personal-savings/monzo-savings.csv"
}

case "$case_name" in
  valid)
    copy_valid_set
    ;;
  duplicates)
    copy_valid_set
    cp "$repo_root/scripts/cert/fixtures/personal-monzo.csv" "$inbox_dir/personal-monzo/monzo-personal-dup.csv"
    cp "$repo_root/scripts/cert/fixtures/business-wise.csv" "$inbox_dir/business-wise/wise-business-dup.csv"
    ;;
  malformed)
    cp "$repo_root/scripts/cert/fixtures/malformed.csv" "$inbox_dir/business-wise/broken.csv"
    ;;
  mixed)
    copy_valid_set
    cp "$repo_root/scripts/cert/fixtures/malformed.csv" "$inbox_dir/business-wise/broken.csv"
    ;;
  empty)
    # Intentionally leave inbox empty
    ;;
  *)
    echo "Unknown fixture case: $case_name" >&2
    echo "Valid cases: valid, duplicates, malformed, mixed, empty" >&2
    exit 2
    ;;
esac

cat <<SUMMARY
Fixture reset complete.
CERT_FIN_HOME=$fin_home
case=$case_name
RESET_DB=${RESET_DB:-1}
SUMMARY
