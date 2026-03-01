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
