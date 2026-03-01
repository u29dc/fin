# Rust Cutover Handoff

## Cutover State

- Branch: `main`
- Active binary install command: `bun run build:cli` (Rust install script)
- Runtime binary target: `$FIN_HOME/fin`
- Archive reference branch: `archive`
- Archive snapshot tag: `archive-snapshot-e2a70c1`

## Validation Evidence

- Rust quality gates pass through commit hooks (`bun run util:check`).
- Parity harness: `./scripts/parity/run_parity.sh` -> `7/7` passing.
- Full command certification: `scripts/parity/certify_commands.sh` -> `30/30` passing.
- TUI smoke: `cargo run -p fin-tui` launches and exits cleanly with `q`.

## Current Runtime Model

- Rust-native:
  - `fin-sdk` foundation (`config/db/rules/health/contracts/units`)
  - `fin-cli` binary entrypoint and native `rules` commands
  - `fin-tui` shell with data-backed overview/transactions/reports routes
- Compatibility delegation:
  - Non-version/non-rules CLI commands delegate to legacy runtime (`bun run packages/cli/src/index.ts ...`) to preserve behavior parity.

## Rollback Paths

1. Fast rollback to archived implementation reference:

```bash
git switch archive
```

2. Restore exact archive snapshot commit:

```bash
git checkout archive-snapshot-e2a70c1
```

3. Return to latest rewrite state:

```bash
git switch main
```

## Rebuild Commands

```bash
bun install
bun run build:cli
FIN_HOME=${FIN_HOME:-$HOME/.tools/fin} "$FIN_HOME/fin" tools --json
```

## Notes

- Personal runtime data and personal rules should remain outside the repository.
- Use `fin.rules.example.toml` as the repository-safe template.
