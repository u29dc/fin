#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fin_home="${PARITY_FIN_HOME:-$repo_root/.tmp/cert-fin-home}"
out_dir="${CERT_OUT_DIR:-$repo_root/.tmp/cert-results}"
bin_path="$fin_home/fin"

mkdir -p "$out_dir"

run_json() {
  local label="$1"
  shift
  FIN_HOME="$fin_home" "$bin_path" "$@" --json > "$out_dir/$label.json"
}

extract_with_bun() {
  local js="$1"
  bun -e "$js"
}

# Stage A: valid fixture flow
PARITY_FIN_HOME="$fin_home" RESET_DB=1 "$repo_root/scripts/parity/reset_fixtures.sh" valid >/dev/null
FIN_HOME="$fin_home" bun run build:cli >/dev/null

run_json tools tools
run_json health_pre_import health
run_json config_show config show
run_json config_validate config validate
run_json rules_show rules show --path "$fin_home/data/fin.rules.toml"
run_json rules_validate rules validate --path "$fin_home/data/fin.rules.toml"
run_json rules_migrate_ts rules migrate-ts
run_json import_valid import
run_json sanitize_discover sanitize discover --unmapped
run_json sanitize_migrate_dry sanitize migrate --dry-run
run_json sanitize_migrate sanitize migrate
run_json sanitize_recategorize_dry sanitize recategorize --dry-run
run_json sanitize_recategorize sanitize recategorize
run_json view_accounts view accounts
run_json view_transactions view transactions --limit=50
run_json view_ledger view ledger --limit=50
run_json view_balance view balance
run_json report_cashflow report cashflow --group=personal --months=6
run_json report_health report health --group=personal --from=2024-01-01
run_json report_runway report runway --group=personal
run_json report_reserves report reserves --group=business
run_json report_categories report categories --group=personal --mode=breakdown --months=6
run_json report_summary report summary
run_json report_audit report audit --account=Expenses:Uncategorized --months=6

entry_id="$(extract_with_bun "const fs=require('fs'); const j=JSON.parse(fs.readFileSync('$out_dir/view_ledger.json','utf8')); const id=j?.data?.entries?.[0]?.id || ''; if(!id){process.exit(1)}; process.stdout.write(id);")"

run_json edit_transaction_dry edit transaction "$entry_id" --description "Certification Updated Description" --dry-run
run_json edit_transaction edit transaction "$entry_id" --description "Certification Updated Description"
run_json view_void_dry view void "$entry_id" --dry-run
run_json view_void view void "$entry_id"

# Stage B: duplicates fixture choreography
PARITY_FIN_HOME="$fin_home" RESET_DB=1 "$repo_root/scripts/parity/reset_fixtures.sh" duplicates >/dev/null
run_json import_duplicates import

# Stage C: empty fixture choreography
PARITY_FIN_HOME="$fin_home" RESET_DB=1 "$repo_root/scripts/parity/reset_fixtures.sh" empty >/dev/null
run_json import_empty import

# Certification summary
extract_with_bun "
const fs = require('fs');
const path = require('path');
const dir = '$out_dir';
const files = fs
  .readdirSync(dir)
  .filter((f) => f.endsWith('.json') && f !== 'cert-summary.json')
  .sort();
const summary = [];
let failed = 0;
for (const file of files) {
  const payload = JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8'));
  const ok = payload?.ok === true;
  if (!ok) failed += 1;
  summary.push({
    file,
    ok,
    tool: payload?.meta?.tool ?? null,
    code: payload?.error?.code ?? null,
    count: payload?.meta?.count ?? null,
  });
}
const report = {
  finHome: '$fin_home',
  outputDir: dir,
  total: summary.length,
  failed,
  passed: summary.length - failed,
  summary,
};
fs.writeFileSync(path.join(dir, 'cert-summary.json'), JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
if (failed > 0) process.exit(1);
"
