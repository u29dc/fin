#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fin_home="${PARITY_FIN_HOME:-$repo_root/.tmp/parity-fin-home}"
action="${1:-}"
snapshot_path="${2:-$repo_root/.tmp/parity-db-snapshot/fin.db}"

db_path="$fin_home/data/fin.db"
mkdir -p "$(dirname "$snapshot_path")"

case "$action" in
  snapshot)
    if [[ ! -f "$db_path" ]]; then
      echo "Database file not found for snapshot: $db_path" >&2
      exit 1
    fi
    cp "$db_path" "$snapshot_path"
    echo "Snapshot saved: $snapshot_path"
    ;;
  restore)
    if [[ ! -f "$snapshot_path" ]]; then
      echo "Snapshot file not found: $snapshot_path" >&2
      exit 1
    fi
    mkdir -p "$(dirname "$db_path")"
    cp "$snapshot_path" "$db_path"
    echo "Database restored: $db_path"
    ;;
  clear)
    rm -f "$db_path"
    echo "Database cleared: $db_path"
    ;;
  *)
    echo "Usage: $0 <snapshot|restore|clear> [snapshot_path]" >&2
    exit 2
    ;;
esac
