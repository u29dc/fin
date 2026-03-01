---
name: fin
description: >-
    Autonomous personal finance analysis powered by the `fin` CLI toolbelt.
    Use this skill to query financial data, analyze spending patterns, check
    runway and reserves, investigate transactions, and produce comprehensive
    financial reports across personal, business, and joint accounts.
compatibility: >-
    Designed for Claude Code with Bash access. Requires runtime files at
    $FIN_HOME/data/fin.config.toml, $FIN_HOME/data/fin.rules.json, and
    $FIN_HOME/data/fin.db.
allowed-tools: Bash Read Write WebSearch
---

## Invocation

`:fin` is the shell alias for the compiled binary:

    :fin <command>

Build the binary if needed:

    bun run build

Use `:fin` in agent workflows. If `:fin` is unavailable in the shell context,
run commands with:

    cargo run -p fin-cli -- <command>

Launch terminal UI when interactive exploration is useful:

    :fin start

Do not use `--json` with `start`; it is interactive-only.

## Orientation

1. Run base checks:

- `:fin tools --json`
- `:fin health --json`
- `:fin config show --json`

2. If health is blocked, follow each check's `fix` guidance.

## Self-Describing CLI

Run `:fin tools --json` when uncertain about command signatures or parameters.
Treat this as the runtime source of truth.

## Common Workflows

### Quick financial snapshot

1. `:fin report summary --json`
2. `:fin report runway --group=personal --json`
3. `:fin report cashflow --group=personal --months=6 --json`

### Investigate spending

1. `:fin report categories --group=personal --mode=median --json`
2. `:fin report audit --account=<category-account-id> --json`
3. `:fin view transactions --group=personal --from=<YYYY-MM-DD> --json`

### Check financial health

1. `:fin report runway --group=personal --json`
2. `:fin report reserves --group=business --json`
3. `:fin report health --group=personal --json`

### Explore in TUI

1. `:fin start`
2. Use `left`/`right` to change routes
3. Use `tab`/`shift+tab` to switch navigation vs main focus
4. Use `cmd+p` or `ctrl+p` to open command palette
5. On Transactions, use `cmd+f` or `ctrl+f` to filter rows
6. Use `r` to refresh and `q` to quit

### Import new data

1. User drops files into `$FIN_HOME/imports/inbox/<folder>/`
2. `:fin import --json`
3. `:fin sanitize discover --unmapped --json`
4. `:fin sanitize migrate --dry-run --json`
5. `:fin sanitize migrate --json`
6. `:fin sanitize recategorize --dry-run --json`
7. `:fin sanitize recategorize --json`

## Rules and Privacy

- Primary rules file: `$FIN_HOME/data/fin.rules.json`.
- Legacy migration source: `$FIN_HOME/data/fin.rules.ts`.
- Keep personal rule sets in home-folder runtime only.
- Repository should contain only sanitized examples (for example `fin.rules.example.json`).

## Expected Output

When returning financial analysis, include:

- Key metrics (runway, net worth, monthly burn, savings rate)
- Trends (month-over-month and period comparisons)
- Actionable insights (anomalies, concentration, reserve gaps)
- Explicit scope (group, accounts, date range, command outputs used)
