# Command Certification Utilities

Utilities for Rust-native command certification with isolated fixture runtime data.

## Safety

These scripts are isolated by default and use `CERT_FIN_HOME`.
They do **not** default to `FIN_HOME` to avoid modifying personal runtime data.

## Commands

```bash
./scripts/cert/reset_fixtures.sh valid
./scripts/cert/certify_commands.sh
```
