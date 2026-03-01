#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
archive_root="${ARCHIVE_ROOT:-/Users/han/Git/fin-archive}"
main_root="${MAIN_ROOT:-$repo_root}"
commands_file="${COMMANDS_FILE:-$repo_root/scripts/parity/commands.txt}"
out_dir="${OUT_DIR:-$repo_root/scripts/parity/out/latest}"
fin_home="${FIN_HOME:-$repo_root/.tmp/parity-fin-home}"

mkdir -p "$out_dir"
mkdir -p "$fin_home"

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required for parity normalization" >&2
  exit 2
fi

run_cmd() {
  local root="$1"
  local cmd="$2"
  local prefix="$3"

  local stdout_file="$out_dir/${prefix}.stdout"
  local stderr_file="$out_dir/${prefix}.stderr"
  local norm_file="$out_dir/${prefix}.normalized.json"
  local meta_file="$out_dir/${prefix}.meta"

  set +e
  (
    cd "$root"
    FIN_HOME="$fin_home" bun run fin $cmd >"$stdout_file" 2>"$stderr_file"
  )
  local code=$?
  set -e

  # Normalize stdout when it is JSON, otherwise wrap as a raw text payload.
  if jq -e . "$stdout_file" >/dev/null 2>&1; then
    jq 'if type == "object" then (if .meta? and (.meta|type=="object") and .meta.elapsed? then .meta.elapsed = 0 else . end) else . end' "$stdout_file" > "$norm_file"
  else
    jq -n --arg raw "$(cat "$stdout_file")" '{raw:$raw}' > "$norm_file"
  fi

  cat > "$meta_file" <<META
exit_code=$code
command=$cmd
root=$root
META

  return 0
}

compare_case() {
  local idx="$1"
  local cmd="$2"
  local id
  id=$(printf "%03d" "$idx")

  local a="archive_${id}"
  local m="main_${id}"

  run_cmd "$archive_root" "$cmd" "$a"
  run_cmd "$main_root" "$cmd" "$m"

  local archive_code
  local main_code
  archive_code=$(awk -F= '/^exit_code=/{print $2}' "$out_dir/${a}.meta")
  main_code=$(awk -F= '/^exit_code=/{print $2}' "$out_dir/${m}.meta")

  local same_code="true"
  if [[ "$archive_code" != "$main_code" ]]; then
    same_code="false"
  fi

  local same_payload="true"
  if ! diff -u "$out_dir/${a}.normalized.json" "$out_dir/${m}.normalized.json" > "$out_dir/case_${id}.diff"; then
    same_payload="false"
  fi

  jq -n \
    --arg id "$id" \
    --arg command "$cmd" \
    --argjson archive_exit "$archive_code" \
    --argjson main_exit "$main_code" \
    --argjson same_code "$same_code" \
    --argjson same_payload "$same_payload" \
    '{id:$id, command:$command, archive_exit:$archive_exit, main_exit:$main_exit, same_code:$same_code, same_payload:$same_payload}'
}

results_file="$out_dir/results.jsonl"
: > "$results_file"

idx=0
while IFS= read -r cmd || [[ -n "$cmd" ]]; do
  [[ -z "$cmd" ]] && continue
  [[ "$cmd" =~ ^# ]] && continue
  idx=$((idx + 1))
  compare_case "$idx" "$cmd" >> "$results_file"
done < "$commands_file"

jq -s '
  {
    generated_at: (now | todateiso8601),
    total: length,
    passing: map(select(.same_code and .same_payload)) | length,
    failing: map(select((.same_code and .same_payload) | not)) | length,
    cases: .
  }
' "$results_file" > "$out_dir/summary.json"

cat "$out_dir/summary.json"
