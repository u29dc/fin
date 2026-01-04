## Documentation

- **README:** `README.md`
- **Svelte/SvelteKit:** [`svelte.dev/llms.txt`](https://svelte.dev/llms.txt), MCP server via `mcp__svelte__*` tools
- **Bun:** [`bun.com/docs/llms.txt`](https://bun.com/docs/llms.txt)
- **Tailwind CSS:** [`tailwindcss.com/docs`](https://tailwindcss.com/docs)
- **ECharts:** [`echarts.apache.org/en/option.html`](https://echarts.apache.org/en/option.html)

## Repository Map

```
.
├── packages/
│   ├── core/src/
│   ├── cli/src/
│   └── web/src/
├── imports/
├── data/
├── package.json
├── biome.json
├── tsconfig.json
└── commitlint.config.js
```

## Commands

- Install: `bun install`
- Dev server: `bun run dev`
- Build/preview: `bun run build`, `bun run preview`
- CLI: `bun run cli <command>` (see CLI section below)
- Import inbox: `bun run import:inbox`
- Sanitization: `bun run sanitize:discover:unmapped`, `bun run sanitize:migrate:dry`, `bun run sanitize:migrate`

## CLI

Thin passthrough layer to core database functionality via `bun run cli <command>`:

- `accounts` - list accounts with balances
- `balance-sheet` - balance sheet from double-entry ledger
- `transactions` - query with filters (account, group, category, date range)
- `ledger` - query journal entries with postings
- `cashflow` - monthly income/expense/savings breakdown
- `import` - run import pipeline
- `sanitize discover|migrate` - description discovery and migration
- `health` - financial health metrics (balance - reserves)
- `runway` - months of cash remaining
- `reserves` - reserve breakdown (tax + expense)
- `categories breakdown|median` - spending by category

Global flags: `--help`, `--db=PATH`, `--format=table|json|tsv`

## Data And Imports

- Import pipeline: scan `imports/inbox/<folder>/` -> parse CSV/PDF -> create journal entries -> dedupe via provider_txn_id -> archive to `imports/archive/`
- Do not commit `imports/`, `data/fin.db`, `data/fin.config.toml`, or `data/fin.rules.ts`

## Core And DB

- Backend logic in `packages/core/src`; SQLite via `bun:sqlite`; schema/migrations in `packages/core/src/db`
- Tables: `chart_of_accounts`, `journal_entries`, `postings`
- Import parsers in `packages/core/src/import/parsers` (monzo, wise, vanguard)
- Config: `packages/core/src/config/` (schema, loader, accessors); loaded via `initConfig()` in hooks.server.ts and CLI entry points

## Web App

- SvelteKit 2 + Svelte 5 in `packages/web`
- SSR load in `+page.server.ts` with direct core imports
- DB singleton in `packages/web/src/lib/server/db.ts`
- Routes: `/`, `/transactions`, `/overview`
- Charts: ECharts-based components (LineChart, SeriesChart, ProjectionChart, Sankey, Treemap)
- Theme toggle in `packages/web/src/lib/ThemeToggle.svelte` with CSS variables

## Quality And Git

- Quality gate after changes: `bun run util:check` (format, lint, types, test)
- Biome scripts: `bun run util:format`, `bun run util:lint`
- TypeScript strict: `bun run util:types`
- Pre-commit: Husky + lint-staged runs `util:check`
- commitlint: `type(scope): subject` with scopes `web`, `core`, `cli`, `db`, `import`, `config`, `deps`, `docs`, `ci`

---

## User Configuration Guide

- **Quick start**: `bun install`, copy `fin.config.template.toml` to `data/fin.config.toml`, copy `fin.rules.template.ts` to `data/fin.rules.ts`, customize accounts, drop CSVs into `imports/inbox/<folder>/`, run `bun run import:inbox` then `bun run dev`
- **Config files**: `data/fin.config.toml` (main config), `data/fin.rules.ts` (transaction mappings), `data/fin.db` (SQLite, auto-created). Copy from templates: `fin.config.template.toml`, `fin.rules.template.ts`
- **Defaults**: GBP in minor units (pence), UK tax rates (25% corp, 20% income, 20% VAT), dividend tax with 500 GBP allowance, parsers for Monzo/Wise/Vanguard, default groups personal/joint/business (all optional)
- **Groups** (`[[groups]]`): `id` (unique identifier), `label` (UI display), `icon` (user/briefcase/heart/building/wallet/piggy-bank), `tax_type` (corp=25%, income=20%, none=0% tax reserve), `expense_reserve_months` (buffer months). Groups order determines UI column order. If omitted, defaults apply
- **Accounts** (`[[accounts]]`): `id` (chart account path), `group` (must match group id), `type` (asset), `provider` (monzo/wise/vanguard - determines CSV parser), optional `label`, `subtype` (checking/savings/investment), `inbox_folder` (folder name in `imports/inbox/` for imports)
- **Financial params** (`[financial]`): `corp_tax_rate` (0.25), `personal_income_tax_rate` (0.20), `vat_rate` (0.20), `expense_reserve_months` (12), `trailing_expense_window_months` (12), `joint_share_you` (0.5). Subsections: `[financial.scenario]` for projected flows, `[financial.investment_projection_annual_returns]` for growth assumptions
- **Transaction rules** (`data/fin.rules.ts`): `rules` array with `patterns`/`target`/`category`, `warnOnUnmapped` (true), `fallbackToRaw` (true). Run `bun run sanitize:discover:unmapped` to find unmapped transactions
