---
name: fin
description: >-
    Autonomous personal finance analysis powered by the `fin` CLI toolbelt.
    Use this skill to query financial data, analyze spending patterns, check
    runway and reserves, investigate transactions, and produce comprehensive
    financial reports across personal, business, and joint accounts.
compatibility: >-
    Designed for Claude Code with Bash access. Requires the installed CLI at
    $HOME/.tools/fin/fin and runtime files at
    $HOME/.tools/fin/data/fin.config.toml,
    $HOME/.tools/fin/data/fin.rules.json, and
    $HOME/.tools/fin/data/fin.db.
allowed-tools: Bash Read Write WebSearch
---

## Invocation

Use the installed binary directly:

    "$HOME/.tools/fin/fin" <command>

If `"$HOME/.tools/fin/fin"` is missing or not executable, return a blocked
prerequisite and stop.

Launch terminal UI when interactive exploration is useful:

    "$HOME/.tools/fin/fin" start

Non-interactive commands emit one JSON envelope to stdout by default.
Use `--text` only when a human-readable view is explicitly needed.
Treat stderr as logs/errors only.
Bare `"$HOME/.tools/fin/fin"` prints clap help; it does not emit JSON.
`"$HOME/.tools/fin/fin" start` is interactive-only.

## Orientation

1. Run base checks:

- `"$HOME/.tools/fin/fin" tools`
- `"$HOME/.tools/fin/fin" health`
- `"$HOME/.tools/fin/fin" config show`

2. If health is blocked, follow each check's `fix` guidance.

## Self-Describing CLI

Run `"$HOME/.tools/fin/fin" tools` when uncertain about command signatures or
parameters.
Treat this as the runtime source of truth.

## Common Workflows

### Quick financial snapshot

1. `"$HOME/.tools/fin/fin" report summary`
2. `"$HOME/.tools/fin/fin" report runway --group=personal`
3. `"$HOME/.tools/fin/fin" report cashflow --group=personal --months=6`

### Investigate spending

1. `"$HOME/.tools/fin/fin" report categories --group=personal --mode=median`
2. `"$HOME/.tools/fin/fin" report audit --account=<category-account-id>`
3. `"$HOME/.tools/fin/fin" view transactions --group=personal --from=<YYYY-MM-DD>`

### Check financial health

1. `"$HOME/.tools/fin/fin" report runway --group=personal`
2. `"$HOME/.tools/fin/fin" report reserves --group=business`
3. `"$HOME/.tools/fin/fin" report health --group=personal`

### Explore in TUI

1. `"$HOME/.tools/fin/fin" start`
2. Use `left`/`right` to change routes
3. Use `tab`/`shift+tab` to switch navigation vs main focus
4. Use `cmd+p` or `ctrl+p` to open command palette
5. On Transactions, use `cmd+f` or `ctrl+f` to filter rows
6. Use `r` to refresh and `q` to quit

### Import new data

1. User drops files into `$HOME/.tools/fin/imports/inbox/<folder>/`
2. `"$HOME/.tools/fin/fin" import`
3. `"$HOME/.tools/fin/fin" sanitize discover --unmapped`
4. `"$HOME/.tools/fin/fin" sanitize migrate --dry-run`
5. `"$HOME/.tools/fin/fin" sanitize migrate`
6. `"$HOME/.tools/fin/fin" sanitize recategorize --dry-run`
7. `"$HOME/.tools/fin/fin" sanitize recategorize`

## Rules and Privacy

- Primary rules file: `$HOME/.tools/fin/data/fin.rules.json`.
- Legacy migration source: `$HOME/.tools/fin/data/fin.rules.ts`.
- Keep personal rule sets in home-folder runtime only.
- Repository should contain only sanitized examples (for example `fin.rules.example.json`).

## Expected Output

When returning financial analysis, include:

- Key metrics (runway, net worth, monthly burn, savings rate)
- Trends (month-over-month and period comparisons)
- Actionable insights (anomalies, concentration, reserve gaps)
- Explicit scope (group, accounts, date range, command outputs used)
