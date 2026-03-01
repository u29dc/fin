## 1. Documentation

- Purpose: operate `fin` as an agent-native personal-finance system with a Rust `sdk/cli/tui` target architecture and parity-preserving command behavior.
- Source priority: executable contracts first, then source files, then prose.
- Runtime truth commands:
  - `:fin tools --json`
  - `:fin health --json`
  - `:fin config show --json`
- JSON-mode contract: in `--json` mode, write exactly one envelope JSON object to stdout and write logs only to stderr.
- Agent skill entry: `.claude/skills/fin/SKILL.md` (symlinked at `.agents/skills/fin`).
- Core references:
  - Rust book: `https://doc.rust-lang.org/book/`
  - SQLite: `https://www.sqlite.org/docs.html`
  - Ratatui: `https://ratatui.rs/`
  - Svelte/SvelteKit (legacy web): `https://svelte.dev/llms.txt`
  - Bun runtime (legacy CLI runtime): `https://bun.sh/docs/llms.txt`

## 2. Repository Structure

```text
.
├── crates/
│   ├── fin-sdk/src/
│   │   ├── config/
│   │   ├── db/
│   │   ├── rules/
│   │   ├── contracts.rs
│   │   ├── error.rs
│   │   ├── health.rs
│   │   └── units.rs
│   ├── fin-cli/src/
│   │   ├── main.rs
│   │   ├── envelope.rs
│   │   ├── registry.rs
│   │   └── commands/
│   └── fin-tui/src/
│       ├── app.rs
│       ├── routes.rs
│       ├── theme.rs
│       └── ui.rs
├── packages/                      # archive-parity legacy TypeScript surfaces
│   ├── cli/src/
│   ├── core/src/
│   └── web/src/
├── scripts/parity/
└── .claude/skills/fin/SKILL.md
```

- External runtime directories (at `$FIN_HOME`):
  - `$FIN_HOME/data/{fin.config.toml,fin.rules.toml,fin.db}` -- config, rules, database.
  - `$FIN_HOME/data/fin.rules.ts` -- optional legacy rules source for migration.
  - `$FIN_HOME/imports/{inbox,archive}` -- CSV/PDF import inbox and archive.
  - `$FIN_HOME/fin` -- compiled binary target.

- Rust source-of-truth modules:
  - Config paths/loader: `crates/fin-sdk/src/config/*`
  - DB schema/migrations: `crates/fin-sdk/src/db/*`
  - Rules schema/loader/migration: `crates/fin-sdk/src/rules/*`
  - Health substrate: `crates/fin-sdk/src/health.rs`

- Legacy parity references:
  - CLI registry: `packages/cli/src/tool.ts`
  - Core domain logic: `packages/core/src/*`
  - Web DB bootstrap: `packages/web/src/lib/server/db.ts`

## 3. Stack

| Layer | Choice | Notes |
| --- | --- | --- |
| Core SDK | Rust (`fin-sdk`) | typed contracts, config/db/rules/health foundations |
| CLI | Rust (`fin-cli`) | primary binary target at `$FIN_HOME/fin` |
| TUI | Rust + Ratatui (`fin-tui`) | cyan theme, dense terminal layout scaffold |
| Storage | SQLite (`rusqlite`) | local ledger database |
| Legacy parity runtime | Bun + TypeScript | command/domain fallback while porting remaining surfaces |
| Web | SvelteKit 2 + Svelte 5 | existing dashboard retained during transition |
| Quality | Biome + tsgo + svelte-check-rs + cargo fmt/clippy/test | dual-surface quality gate |

- Rules file direction:
  - Primary: `$FIN_HOME/data/fin.rules.toml`
  - Migration source: `$FIN_HOME/data/fin.rules.ts`

## 4. Commands

- Bootstrap: `bun install`.
- Build Rust binary (default): `bun run build:cli`.
- Build legacy TS binary (fallback): `bun run build:cli:ts`.
- Verify install path: `bun run util:verify:cli-path`.
- Rust dev run: `cargo run -p fin-cli -- <command>`.
- Legacy dev run: `bun run fin <command>`.
- Web dev/build: `bun run dev`, `bun run build`, `bun run preview`.
- Full quality gate: `bun run util:check`.

- Runtime commands (agent workflows):
  - `fin tools`
  - `fin health`
  - `fin config show|validate`
  - `fin import`
  - `fin sanitize discover|migrate|recategorize`
  - `fin view accounts|transactions|ledger|balance|void`
  - `fin edit transaction <id>`
  - `fin report cashflow|health|runway|reserves|categories|audit|summary`

- Common flags: `--json`, `--db`, `--format`.
- Common filters: `--group`, `--from`, `--to`, `--months`, `--limit`, `--account`.
- Safety flags: `--dry-run`, `--verbose` for sanitize/mutation previews.

## 5. Architecture

- System shape: Rust-first monorepo architecture with staged parity bridge.
- Boundaries:
  - `fin-sdk` (`crates/fin-sdk`): contracts, config path precedence, DB connection policy, migrations, TOML rules, health checks.
  - `fin-cli` (`crates/fin-cli`): command entrypoint, envelopes, exit codes, tool metadata, compatibility routing.
  - `fin-tui` (`crates/fin-tui`): Ratatui app shell and route/state/theme system.
  - `packages/*`: legacy reference/runtime used for parity cross-checking and temporary command delegation.

- Compatibility behavior (current):
  - `fin-cli` handles `version` natively.
  - Non-version commands are delegated to `bun run packages/cli/src/index.ts ...` to preserve command parity while remaining Rust port tickets are completed.
  - Exit codes and JSON envelopes remain tool-compatible.

- Config and path resolution (Rust SDK):
  - FIN home precedence: `FIN_HOME` -> `TOOLS_HOME/fin` -> `$HOME/.tools/fin`.
  - Config path: explicit -> `FIN_CONFIG_PATH` -> `$FIN_HOME/data/fin.config.toml`.
  - DB path: explicit (`--db`) -> `DB_PATH` -> config dir `fin.db` -> `$FIN_HOME/data/fin.db`.
  - Rules path: explicit -> `FIN_RULES_PATH` -> config `sanitization.rules` -> `$FIN_HOME/data/fin.rules.toml`.

- Ledger model targets:
  - `chart_of_accounts`
  - `journal_entries`
  - `postings`
  - dedupe key `(provider_txn_id, account_id)` where provider id exists.

- Migration model:
  - `PRAGMA user_version` with Rust SDK target `SCHEMA_VERSION = 5`.
  - migration metadata recorded in `crates/fin-sdk/src/db/schema.rs`.

- Data handling rules:
  - Never commit runtime state from `data/` or `imports/`.
  - Never commit personal rules from home-folder runtime.
  - Keep only sanitized examples in repository.

## 6. Quality

- Required completion gates:
  - zero TypeScript type errors
  - zero lint warnings
  - passing TS and Rust tests
  - successful Rust workspace checks/build

- Standard checks:
  - `bun run util:format`
  - `bun run util:lint`
  - `bun run util:types`
  - `bun run util:types:svelte`
  - `bun run util:types:zvelte`
  - `bun test`
  - `bun run util:rust:fmt:check`
  - `bun run util:rust:lint`
  - `bun run util:rust:check`
  - `bun run util:rust:test`
  - `bun run util:check`

- Manual validation checklist:
  - Run `:fin tools --json`, `:fin health --json`, `:fin config show --json`.
  - Use isolated test home (`PARITY_FIN_HOME`) for import/sanitize mutation checks.
  - For mutations, run `--dry-run` before applying.
  - Verify group/date filters before analysis output.

- Risks to surface in agent output:
  - provider file format drift
  - mapping/rules gaps causing uncategorized leakage
  - partial parity if a command is still on delegated legacy path
