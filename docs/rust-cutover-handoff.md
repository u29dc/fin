# Rust Cutover Handoff

## Cutover State

- Branch: `main`
- Active binary install command: `bun run build` (Rust install script)
- Runtime binary target: `$FIN_HOME/fin`
- Archive reference branch: `archive`
- Archive snapshot tag: `archive-snapshot-e2a70c1`
- Rust cutover tag: `rust-cutover-20260301`
- Strict-native final tag: `rust-strict-native-20260301`

## Validation Evidence

- Rust quality gates pass through commit hooks (`bun run util:check`).
- TUI smoke:
  - `cargo run -p fin-tui` launches and exits cleanly with `q`
  - route switching works on `tab`/`shift+tab` and `left`/`right`
  - command palette opens with `cmd+p` / `ctrl+p`

## Current Runtime Model

- Rust-native:
  - `fin-sdk` foundation (`config/db/rules/health/contracts/units/import/sanitize/queries/reports/mutations`)
  - `fin-cli` binary entrypoint and native command surface
  - `fin-tui` Ratatui shell using Rust runtime data fetch paths

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
bun run build
FIN_HOME=${FIN_HOME:-$HOME/.tools/fin} "$FIN_HOME/fin" tools --json
```

## Notes

- Personal runtime data and personal rules should remain outside the repository.
- Use `fin.rules.example.json` as the repository-safe template.
