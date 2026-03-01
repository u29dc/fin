#!/usr/bin/env bash
set -euo pipefail

expected_dir="${FIN_HOME:-${TOOLS_HOME:-$HOME/.tools}/fin}"
expected_bin="$expected_dir/fin"

if [[ ! -x "$expected_bin" ]]; then
  echo "missing executable fin binary at: $expected_bin" >&2
  exit 1
fi

if command -v :fin >/dev/null 2>&1; then
  echo "alias command ':fin' detected"
else
  echo "note: ':fin' alias is not available in this shell context" >&2
fi

echo "verified fin binary path: $expected_bin"
