---
name: fin
description: >-
    Autonomous personal finance analysis powered by the `fin` CLI toolbelt.
    Use this skill to query financial data, analyze spending patterns, check
    runway and reserves, investigate transactions, and produce comprehensive
    financial reports across personal, business, and joint accounts.
compatibility: >-
    Designed for Claude Code with Bash access. Requires fin config at
    data/fin.config.toml and database at data/fin.db.
allowed-tools: Bash Read Write WebSearch
---

## Orientation

1. Run the base checks:

- `bun run fin tools --json`
- `bun run fin health --json`
- `bun run fin config show --json`

If health is blocked, follow the fix guidance in each check's `fix` field.

## Self-describing CLI

Run `bun run fin tools --json` whenever you're uncertain about parameters
or command signatures. Treat it as the source of truth.

## Common workflows

### Quick financial snapshot

1. `bun run fin report summary --json` -- comprehensive overview
2. Analyze the data and present key metrics to the user

### Investigate spending

1. `bun run fin report categories --group=personal --mode=median --json`
2. `bun run fin report audit --account=<category> --json` for drill-down
3. `bun run fin view transactions --group=personal --from=<date> --json`

### Check financial health

1. `bun run fin report runway --group=personal --json` -- how long cash lasts
2. `bun run fin report reserves --group=business --json` -- tax/expense reserves
3. `bun run fin report cashflow --group=personal --months=6 --json`

### Import new data

1. User drops CSVs into `imports/inbox/<folder>/`
2. `bun run fin import --json`
3. `bun run fin sanitize discover --unmapped --json` -- check for unmapped
4. `bun run fin sanitize migrate --dry-run --json` -- preview rule application
5. `bun run fin sanitize migrate --json` -- apply rules

## Expected output

When the user asks you to analyze their finances, return:

- Key metrics (runway, net worth, monthly burn, savings rate)
- Trends (MoM changes, seasonal patterns)
- Actionable insights (spending anomalies, categorization gaps)
- Clear data source references (which accounts, what time period)
