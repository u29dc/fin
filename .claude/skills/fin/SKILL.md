---
name: fin
description: >-
    Autonomous personal finance analysis powered by the `fin` CLI toolbelt.
    Use this skill to query financial data, analyze spending patterns, check
    runway and reserves, investigate transactions, and produce comprehensive
    financial reports across personal, business, and joint accounts.
compatibility: >-
    Designed for Claude Code with Bash access. Requires fin config at
    $FIN_HOME/data/fin.config.toml and database at $FIN_HOME/data/fin.db.
allowed-tools: Bash Read Write WebSearch
---

## Invocation

`:fin` is a shell alias for the compiled binary. Use it directly in bash:

    :fin <command>

NEVER use `bun run fin` in agent workflows -- that is the dev entrypoint.

If `:fin` is not found, build it first: `bun run build:cli` (in the repo root).

## Orientation

> If `:fin` is not found, run `bun run build:cli`.

1. Run the base checks:

- `:fin tools --json`
- `:fin health --json`
- `:fin config show --json`

If health is blocked, follow the fix guidance in each check's `fix` field.

## Self-describing CLI

Run `:fin tools --json` whenever you're uncertain about parameters
or command signatures. Treat it as the source of truth.

## Common workflows

### Quick financial snapshot

1. `:fin report summary --json` -- comprehensive overview
2. Analyze the data and present key metrics to the user

### Investigate spending

1. `:fin report categories --group=personal --mode=median --json`
2. `:fin report audit --account=<category> --json` for drill-down
3. `:fin view transactions --group=personal --from=<date> --json`

### Check financial health

1. `:fin report runway --group=personal --json` -- how long cash lasts
2. `:fin report reserves --group=business --json` -- tax/expense reserves
3. `:fin report cashflow --group=personal --months=6 --json`

### Import new data

1. User drops CSVs into `$FIN_HOME/imports/inbox/<folder>/`
2. `:fin import --json`
3. `:fin sanitize discover --unmapped --json` -- check for unmapped
4. `:fin sanitize migrate --dry-run --json` -- preview rule application
5. `:fin sanitize migrate --json` -- apply rules

## Expected output

When the user asks you to analyze their finances, return:

- Key metrics (runway, net worth, monthly burn, savings rate)
- Trends (MoM changes, seasonal patterns)
- Actionable insights (spending anomalies, categorization gaps)
- Clear data source references (which accounts, what time period)
