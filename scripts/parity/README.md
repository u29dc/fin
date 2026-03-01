# Parity Utilities

Utilities for archive/main parity checks and command certification.

## Safety

These scripts are isolated by default and use `PARITY_FIN_HOME`.
They do **not** default to `FIN_HOME` to avoid modifying personal runtime data.

## Commands

```bash
./scripts/parity/extract_archive_baseline.sh
./scripts/parity/run_parity.sh
./scripts/parity/reset_fixtures.sh valid
./scripts/parity/db_snapshot.sh snapshot
```
