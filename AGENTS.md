## 1. Documentation

- **Framework**: [`svelte.dev/llms.txt`](https://svelte.dev/llms.txt), MCP via `mcp__svelte__*`
- **UI**: [`tailwindcss.com/docs`](https://tailwindcss.com/docs)
- **Data**: [`echarts.apache.org/en/option.html`](https://echarts.apache.org/en/option.html)
- **DevTools**: [`bun.com/docs/llms.txt`](https://bun.com/docs/llms.txt)

## 2. Repository Structure

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

## 3. Stack

| Layer     | Choice         | Notes                                                               |
| --------- | -------------- | ------------------------------------------------------------------- |
| Framework | SvelteKit 2    | Svelte 5 in `packages/web`                                          |
| Database  | SQLite         | Via `bun:sqlite`, schema in `packages/core/src/db`                  |
| Charts    | ECharts        | LineChart, SeriesChart, ProjectionChart, Sankey, Treemap components |
| Runtime   | Bun            | Package manager, script runner, native SQLite                       |
| Linting   | Biome          | Format + lint                                                       |
| Monorepo  | Bun workspaces | packages/core, packages/cli, packages/web                           |

## 4. Commands

- `bun install` - Install dependencies
- `bun run dev` - Start dev server
- `bun run build` / `bun run preview` - Build and preview
- `bun run fin <command>` - CLI commands (see CLI section)
- `bun run import:inbox` - Run import pipeline
- `bun run sanitize:discover:unmapped` / `bun run sanitize:migrate:dry` / `bun run sanitize:migrate` - Transaction sanitization

## 5. Architecture

- **Monorepo**: `packages/core` (backend logic, SQLite, schema/migrations), `packages/cli` (thin passthrough to core), `packages/web` (SvelteKit frontend)
- **Database**: Tables `chart_of_accounts`, `journal_entries`, `postings`; import parsers in `packages/core/src/import/parsers` (monzo, wise, vanguard)
- **Config**: `packages/core/src/config/` (schema, loader, accessors), loaded via `initConfig()` in hooks.server.ts and CLI entry points
- **Web**: SSR load in `+page.server.ts` with direct core imports, DB singleton in `packages/web/src/lib/server/db.ts`, routes `/`, `/transactions`, `/overview`, theme toggle with CSS variables
- **Imports**: Pipeline scans `imports/inbox/<folder>/` → parse CSV/PDF → create journal entries → dedupe via provider_txn_id → archive to `imports/archive/`; do not commit `imports/`, `data/fin.db`, `data/fin.config.toml`, or `data/fin.rules.ts`

## 6. CLI

Commands via `bun run fin <command>`:

- `accounts` - List accounts with balances
- `balance-sheet` - Balance sheet from double-entry ledger
- `transactions` - Query with filters (account, group, category, date range)
- `ledger` - Query journal entries with postings
- `cashflow` - Monthly income/expense/savings breakdown
- `import` - Run import pipeline
- `sanitize discover|migrate` - Description discovery and migration
- `health` - Financial health metrics (balance - reserves)
- `runway` - Months of cash remaining
- `reserves` - Reserve breakdown (tax + expense)
- `categories breakdown|median` - Spending by category

Global flags: `--help`, `--db=PATH`, `--format=table|json|tsv`

## 7. Configuration

- **Quick start**: `bun install`, copy `fin.config.template.toml` → `data/fin.config.toml`, copy `fin.rules.template.ts` → `data/fin.rules.ts`, customize accounts, drop CSVs into `imports/inbox/<folder>/`, run `bun run import:inbox` then `bun run dev`
- **Config files**: `data/fin.config.toml` (main config), `data/fin.rules.ts` (transaction mappings), `data/fin.db` (SQLite, auto-created)
- **Defaults**: GBP in minor units (pence), UK tax rates (25% corp, 20% income, 20% VAT), dividend tax with 500 GBP allowance, parsers for Monzo/Wise/Vanguard, default groups personal/joint/business
- **Groups** (`[[groups]]`): `id`, `label`, `icon` (user/briefcase/heart/building/wallet/piggy-bank), `tax_type` (corp=25%, income=20%, none=0%), `expense_reserve_months`; order determines UI column order
- **Accounts** (`[[accounts]]`): `id` (chart account path), `group`, `type` (asset), `provider` (monzo/wise/vanguard), optional `label`, `subtype`, `inbox_folder`
- **Financial** (`[financial]`): `corp_tax_rate`, `personal_income_tax_rate`, `vat_rate`, `expense_reserve_months`, `trailing_expense_window_months`, `joint_share_you`; subsections for `[financial.scenario]` and `[financial.investment_projection_annual_returns]`
- **Rules** (`data/fin.rules.ts`): `rules` array with `patterns`/`target`/`category`, `warnOnUnmapped`, `fallbackToRaw`; run `bun run sanitize:discover:unmapped` to find unmapped transactions

## 8. Quality

- Quality gate after changes: `bun run util:check` (format, lint, types, test)
- Biome scripts: `bun run util:format`, `bun run util:lint`
- TypeScript strict: `bun run util:types`
- Pre-commit: Husky + lint-staged runs `util:check`
- Commits: Always use Conventional Commits format `type(scope): description` with body required, format as `type(scope): description` then newline then body with `- Item` bullets explaining the "why"; if commitlint.config.js exists read allowed types/scopes from there, otherwise use logical types (feat/fix/refactor/docs/chore/test) and derive scope from the area being modified
