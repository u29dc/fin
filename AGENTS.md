## 1. Documentation

- Purpose: operate `fin` as an agent-native personal-finance system with Rust-native `sdk/cli/tui` runtime on `main`.
- Source priority: executable contracts first, then source files, then prose docs.
- Runtime truth commands:
    - `:fin tools`
    - `:fin health`
    - `:fin config show`
- JSON contract: non-interactive commands print exactly one envelope JSON object to stdout by default; logs/errors go to stderr. Use `--text` for human-readable output. `fin start` is interactive-only.
- Agent skill entry: `.claude/skills/fin/SKILL.md` (mirrored via `.agents/skills/fin`).
- Primary references:
    - Rust: `https://doc.rust-lang.org/book/`
    - SQLite: `https://www.sqlite.org/docs.html`
    - Ratatui: `https://ratatui.rs/`

## 2. Repository Structure

```text
.
├── crates/
│   ├── fin-sdk/src/
│   │   ├── config/
│   │   ├── db/
│   │   ├── rules/
│   │   ├── categories.rs
│   │   ├── import.rs
│   │   ├── sanitize.rs
│   │   ├── queries.rs
│   │   ├── reports.rs
│   │   └── mutations.rs
│   ├── fin-cli/src/
│   │   ├── main.rs
│   │   ├── registry.rs
│   │   ├── envelope.rs
│   │   └── commands/
│   └── fin-tui/src/
│       ├── app.rs
│       ├── fetch.rs
│       ├── routes.rs
│       ├── theme.rs
│       └── ui.rs
└── .claude/skills/fin/SKILL.md
```

- External runtime directories (`$FIN_HOME`):
    - `$FIN_HOME/data/{fin.config.toml,fin.rules.json,fin.db}`
    - `$FIN_HOME/data/fin.rules.ts` (migration source only for `fin rules migrate-ts`)
    - `$FIN_HOME/imports/{inbox,archive}`
    - `$FIN_HOME/fin` (installed binary)

- Active Rust source-of-truth modules:
    - Config + paths: `crates/fin-sdk/src/config/*`
    - DB schema/migrations/seeding: `crates/fin-sdk/src/db/*`
    - Rules schema/loader/migration: `crates/fin-sdk/src/rules/*`
    - Domain logic: `crates/fin-sdk/src/{import,sanitize,queries,reports,mutations}.rs`

## 3. Stack

| Layer        | Choice                       | Notes                              |
| ------------ | ---------------------------- | ---------------------------------- |
| SDK          | Rust (`fin-sdk`)             | domain/data logic                  |
| CLI          | Rust (`fin-cli`)             | primary agent command surface      |
| TUI          | Rust + Ratatui (`fin-tui`)   | cyan, dense terminal UI            |
| Storage      | SQLite (`rusqlite`)          | local ledger database              |
| Repo tooling | Bun scripts + commit tooling | packaging/check orchestration only |

- Rules direction:
    - Primary runtime rules: JSON (`$FIN_HOME/data/fin.rules.json`)
    - Optional migration source: TS (`$FIN_HOME/data/fin.rules.ts`)

## 4. Commands

- Install binary: `bun run build`.
- Dev run CLI: `cargo run -p fin-cli -- <command>`.
- Dev run TUI: `cargo run -p fin-tui --`.
- Runtime TUI: `:fin start`.
- Full quality gate: `bun run util:check`.

- Runtime command groups:
    - `fin tools`
    - `fin health`
    - `fin config show|validate`
    - `fin rules show|validate|migrate-ts`
    - `fin import`
    - `fin sanitize discover|migrate|recategorize`
    - `fin view accounts|transactions|ledger|balance|void`
    - `fin edit transaction <id>`
    - `fin report cashflow|health|runway|reserves|categories|audit|summary`
    - `fin start` (launch Ratatui terminal UI, interactive-only)

- TUI key contract:
    - `left`/`right` switch routes
    - `tab`/`shift+tab` switch focus between navigation and main pane
    - `1/2/3/4/5/6` jump to summary/transactions/cashflow/overview/categories/reports
    - `cmd+f` or `ctrl+f` starts transactions filter mode on Transactions route
    - `cmd+p` or `ctrl+p` opens command palette
    - `r` refreshes current route
    - `q` exits TUI

- Common flags: `--text`, `--db`, `--format`.
- Common filters: `--group`, `--from`, `--to`, `--months`, `--limit`, `--account`.
- Mutation safety flags: `--dry-run`.

## 5. Architecture

- System shape: Rust-only runtime on `main`.
- Boundaries:
    - `fin-sdk`: all financial domain behavior (ingestion, sanitize, queries/reports, mutations, config/db/rules/health).
    - `fin-cli`: clap command tree + envelope + exit code contracts + tool metadata.
    - `fin-tui`: Ratatui app shell/routes/theme; fetches read models directly through Rust SDK services.

- Path precedence:
    - FIN home: `FIN_HOME` -> `TOOLS_HOME/fin` -> `$HOME/.tools/fin`.
    - Config: explicit -> `FIN_CONFIG_PATH` -> `$FIN_HOME/data/fin.config.toml`.
    - DB: explicit (`--db`) -> `DB_PATH` -> config dir `fin.db` -> `$FIN_HOME/data/fin.db`.
    - Rules: explicit -> `FIN_RULES_PATH` -> config `sanitization.rules` -> `$FIN_HOME/data/fin.rules.json`.

- Ledger model:
    - `chart_of_accounts`
    - `journal_entries`
    - `postings`
    - Dedupe key `(provider_txn_id, account_id)` where provider id exists.

- Data handling constraints:
    - Never commit runtime state from `data/` or `imports/`.
    - Never commit personal rules from home-directory runtime files.
    - Keep only sanitized template examples in repository.

## 6. Quality

- Required completion gates:
    - zero Rust compile errors/warnings
    - passing Rust tests
    - successful workspace build/install command

- Standard checks:
    - `bun run util:format`
    - `bun run util:lint`
    - `bun run util:types`
    - `bun run util:test`
    - `bun run util:build`
    - `bun run util:check`

- Manual validation checklist:
    - Run `:fin tools`, `:fin health`, `:fin config show`.
    - Run `--dry-run` before mutating commands.
    - Confirm group/date filters before interpreting report outputs.

- Risks to surface explicitly:
    - provider file format drift (CSV/PDF changes)
    - insufficient rules causing uncategorized leakage
    - local runtime data missing/stale causing partial analytics
