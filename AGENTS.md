## 1. Documentation

- Purpose: operate `fin` as an agent-native personal-finance system with CLI-first workflows and a SvelteKit web layer.
- Source priority: executable contracts first, then source files, then prose.
- Runtime truth commands:
    - `:fin tools --json`
    - `:fin health --json`
    - `:fin config show --json`
- JSON-mode contract: in `--json` mode, write exactly one envelope JSON object to stdout and write logs only to stderr.
- Agent skill entry: `.claude/skills/fin/SKILL.md`.
- Core references:
    - Svelte/SvelteKit: `https://svelte.dev/llms.txt`
    - Runtime: `https://bun.sh/docs/llms.txt`
    - Validation: `https://zod.dev/llms.txt`
    - UI utility: `https://tailwindcss.com/docs`
    - Charts: `https://echarts.apache.org/en/option.html`

## 2. Repository Structure

```text
.
├── packages/
│   ├── cli/src/
│   │   ├── index.ts
│   │   ├── main.ts
│   │   ├── envelope.ts
│   │   ├── tool.ts
│   │   └── commands/{config,edit,health,import,report,sanitize,tools,view}
│   ├── cli/tests/
│   ├── core/src/
│   │   ├── config/
│   │   ├── db/
│   │   ├── import/{parsers/*,journal.ts,scanner.ts}
│   │   ├── sanitize/
│   │   ├── queries/
│   │   └── utils/
│   ├── core/tests/
│   └── web/src/
│       ├── hooks.server.ts
│       ├── lib/server/db.ts
│       └── routes/{+,overview,transactions}
```

- External runtime directories (at `$FIN_HOME`):
    - `$FIN_HOME/data/{fin.config.toml,fin.rules.ts,fin.db}` -- config, rules, database.
    - `$FIN_HOME/imports/{inbox,archive}` -- CSV/PDF import inbox and archive.
    - `$FIN_HOME/fin` -- compiled binary.

- CLI command registry source of truth: `packages/cli/src/tool.ts`.
- CLI envelope contract source of truth: `packages/cli/src/envelope.ts`.
- Finance schema/migration source of truth: `packages/core/src/db/schema.ts`, `packages/core/src/db/migrate.ts`.
- Runtime config loader source of truth: `packages/core/src/config/loader.ts`.
- Web DB bootstrap source of truth: `packages/web/src/lib/server/db.ts`.

## 3. Stack

| Layer         | Choice                                    | Notes                             |
| ------------- | ----------------------------------------- | --------------------------------- |
| Framework     | SvelteKit 2 + Svelte 5                    | web UI and SSR data loads         |
| Runtime       | Bun                                       | workspace scripts, CLI execution  |
| Language      | TypeScript                                | strict-mode monorepo              |
| Validation    | Zod 4                                     | config and schema validation      |
| CLI           | citty                                     | command tree + typed args         |
| Storage       | SQLite (`bun:sqlite`)                     | local ledger database             |
| Visualization | ECharts                                   | overview, runway, category charts |
| Quality       | Biome + tsgo + svelte-check-rs + bun test | lint, types, tests                |

- Runtime local files: `$FIN_HOME/data/fin.config.toml`, `$FIN_HOME/data/fin.rules.ts`, `$FIN_HOME/data/fin.db`.
- Import filesystem roots: `$FIN_HOME/imports/inbox/` and `$FIN_HOME/imports/archive/`.

## 4. Commands

- Bootstrap: `bun install`.
- Agent entrypoint: `:fin <command>` (shell alias for compiled binary).
- Dev entrypoint: `bun run fin <command>` (Bun dev runtime; development only).
- Web dev server: `bun run dev`.
- Web production build/preview: `bun run build && bun run preview`.
- Compiled binary build: `bun run build:cli` (output: `$FIN_HOME/fin`).
- Full quality gate: `bun run util:check`.

- Infrastructure command `fin tools`: capability discovery from `toolRegistry[]`; supports detail lookup by tool name.
- Infrastructure command `fin health`: config, config validation, DB presence/schema, rules file, and inbox checks.

- Tool `fin config show`: load and display parsed config with groups/accounts/financial sections.
- Tool `fin config validate`: validate config and return structured errors.
- Tool `fin import`: scan inbox, parse provider files, dedupe transactions, create journal entries, archive processed files.
- Tool `fin sanitize discover`: profile description patterns and identify unmapped values.
- Tool `fin sanitize migrate`: apply description sanitization rules to journal entries.
- Tool `fin sanitize recategorize`: reclassify uncategorized expense postings using mapping rules.
- Tool `fin view accounts`: account balances and account metadata views.
- Tool `fin view transactions`: filtered transaction list with search, dates, account, and group filters.
- Tool `fin view ledger`: journal entries with postings and account-level filtering.
- Tool `fin view balance`: balance sheet and net-worth breakdown.
- Tool `fin edit transaction <id>`: edit a journal entry's description and/or expense category with atomic updates and auto-account creation.
- Tool `fin view void <id>`: create reversing journal entry for a bad import entry.
- Tool `fin report cashflow`: monthly income/expenses/net/savings rate series.
- Tool `fin report health`: financial health metric series.
- Tool `fin report runway`: runway projections by group or consolidated selection.
- Tool `fin report reserves`: reserve requirements and coverage.
- Tool `fin report categories`: spending breakdown or monthly median by category.
- Tool `fin report audit`: payee drill-down for a target expense account.
- Tool `fin report summary`: comprehensive multi-section overview payload.

- Common flags: `--json`, `--db`, `--format`.
- Common filters: `--group`, `--from`, `--to`, `--months`, `--limit`, `--account`.
- Safety flags: `--dry-run`, `--verbose` for sanitize and mutation previews.

- Recommended analytics loop:

```bash
:fin tools --json
:fin health --json
:fin config show --json
:fin report summary --json
:fin report runway --group=personal --json
:fin report categories --group=personal --mode=breakdown --months=6 --json
:fin view transactions --group=personal --limit=50 --json
```

- Recommended import/sanitize loop:

```bash
:fin import --json
:fin sanitize discover --unmapped --json
:fin sanitize migrate --dry-run --json
:fin sanitize migrate --json
:fin sanitize recategorize --dry-run --json
:fin sanitize recategorize --json
```

## 5. Architecture

- System shape: monorepo with `core` for domain/data logic, `cli` for command surface, `web` for UI.
- Boundaries:
    - `@fin/core`: config, DB schema/migrations, import pipeline, sanitize pipeline, query/report logic.
    - `@fin/cli`: citty command tree with JSON envelope contract and tool registry metadata.
    - `@fin/web`: SvelteKit routes that load from `@fin/core` queries using a server-side DB singleton.

- Envelope contract:
    - Success: `{ ok: true, data, meta: { tool, elapsed, count?, total?, hasMore? } }`
    - Error: `{ ok: false, error: { code, message, hint }, meta: { tool, elapsed } }`
    - Exit codes: `0` success, `1` runtime error, `2` blocked prerequisites.

- Config and path resolution:
    - Config lookup precedence: explicit path -> `FIN_CONFIG_PATH` -> `resolveFinPaths().configFile` (FIN_HOME -> TOOLS_HOME -> `$HOME/.tools/fin`).
    - DB lookup precedence in CLI: `--db` -> `DB_PATH` -> config-directory `fin.db`.
    - Rules lookup: `$FIN_HOME/data/fin.rules.ts` (or configured rules path) merged over generic defaults.

- Ledger data model:
    - `chart_of_accounts`: hierarchical account tree and account metadata.
    - `journal_entries`: entry header including dates, descriptions, transfer marker, source file.
    - `postings`: debit/credit lines with account IDs, amounts, currency, provider IDs.
    - Dedup key: unique `(provider_txn_id, account_id)` where provider ID exists.

- Migration model:
    - Schema version in `PRAGMA user_version`, current target `SCHEMA_VERSION` from `packages/core/src/db/schema.ts`.
    - Migrations run to latest on writable and readonly DB opens where configured.

- Import pipeline contract:
    - Scan `$FIN_HOME/imports/inbox/*` folders mapped to configured chart account IDs.
    - Detect provider/parser from folder mapping and file headers.
    - Parse Monzo/Wise/Vanguard CSV or Vanguard PDF.
    - Canonicalize and sanitize descriptions via rules loader.
    - Deduplicate transactions and auto-detect transfer pairs.
    - Write journal/postings atomically and archive processed files.

- Sanitization contract:
    - Discover mode summarizes description frequency/amount signals.
    - Migrate mode updates descriptions only when safety conditions pass.
    - Recategorize mode reassigns uncategorized postings using mapping rules.

- Web layer contract:
    - `packages/web/src/hooks.server.ts` initializes config for server context.
    - `packages/web/src/lib/server/db.ts` opens DB and migrates to latest.
    - Route loads provide batched account, cashflow, runway, reserve, and transaction datasets.

- Data handling rules:
    - Treat `$FIN_HOME/data/` and `$FIN_HOME/imports/` as local state; do not commit generated runtime artifacts.
    - Surface timeframe, group scope, and account scope whenever reporting metrics.

## 6. Quality

- Required completion gates:
    - zero type errors
    - zero linter warnings
    - passing tests
    - successful builds for changed surfaces (`bun run build`, `bun run build:cli`)

- Standard checks:
    - `bun run util:format`
    - `bun run util:lint`
    - `bun run util:types`
    - `bun run util:types:svelte`
    - `bun run util:types:zvelte`
    - `bun test`
    - `bun run util:check`

- Test surface:
    - CLI: contract, registry, parity-gate harness.
    - Core: import parser, journal/transfer logic, sanitize matcher/migrator, query metrics/groups.
    - Web: validate route behavior via integration checks when UI/server logic changes.

- Manual validation checklist:
    - Run `:fin health --json` before financial analysis.
    - Verify config/rules paths before import and sanitize workflows.
    - For mutation commands, run `--dry-run` first.
    - For analytics output, verify group and period filters match the user request.

- Risks to surface in agent output:
    - incomplete mapping rules causing uncategorized leakage
    - provider file format drift breaking parser assumptions
    - stale or missing local data files producing partial analytics
