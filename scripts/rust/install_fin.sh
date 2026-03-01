#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
out_dir="${FIN_HOME:-${TOOLS_HOME:-$HOME/.tools}/fin}"
out_bin="$out_dir/fin"

cd "$repo_root"
cargo build --release -p fin-cli -p fin-tui

mkdir -p "$out_dir"
cp "$repo_root/target/release/fin" "$out_bin"
cp "$repo_root/target/release/fin-tui" "$out_dir/fin-tui"
chmod +x "$out_bin"
chmod +x "$out_dir/fin-tui"

echo "installed rust fin binaries to: $out_dir"
