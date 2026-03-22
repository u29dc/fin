> `fin` is a local-first finance workspace: Rust SDK, CLI, TUI, and read-only daemon plus a Bun/SvelteKit dashboard. `fin-sdk` owns financial logic; every other surface should stay thin over it.

## 1. Documentation

- Primary source files: [`crates/fin-sdk/src/lib.rs`](crates/fin-sdk/src/lib.rs), [`crates/fin-cli/src/main.rs`](crates/fin-cli/src/main.rs), [`crates/fin-api/src/api.rs`](crates/fin-api/src/api.rs), [`packages/web/src/lib/server/api.ts`](packages/web/src/lib/server/api.ts)
- Runtime/operator docs: [`docs/fin-api.md`](docs/fin-api.md), [`docs/benchmarks/api-007-fin-api.md`](docs/benchmarks/api-007-fin-api.md), [`docs/benchmarks/qa-002-final.md`](docs/benchmarks/qa-002-final.md)
- Roadmap context: [`PLAN.md`](PLAN.md), [`tickets.csv`](tickets.csv), [`tests/parity/archive-parity-matrix.csv`](tests/parity/archive-parity-matrix.csv)
- Templates and sanitized examples: [`fin.config.template.toml`](fin.config.template.toml), [`fin.rules.example.json`](fin.rules.example.json), [`tests/fixtures/benchmark/`](tests/fixtures/benchmark/)
- Installed-runtime skill notes: [`.claude/skills/fin/SKILL.md`](.claude/skills/fin/SKILL.md)

## 2. Repository Structure

```text
.
├── crates/
│   ├── fin-sdk/            core finance logic, config, db, rules, fixtures
│   ├── fin-cli/            clap CLI and JSON/text envelope surface
│   ├── fin-api/            read-only axum daemon over fin-sdk
│   └── fin-tui/            ratatui app using shared SDK read models
├── packages/web/           SvelteKit dashboard backed by fin-api
├── scripts/                dev orchestration and benchmark runners
├── docs/benchmarks/        recorded perf/QA notes plus generated evidence
├── tests/fixtures/benchmark/ committed synthetic fixture source
└── AGENTS.md               canonical repo-level agent instructions
```

- Start domain changes in [`crates/fin-sdk/src/`](crates/fin-sdk/src/) and keep CLI, API, TUI, and web adapters thin.
- Treat [`crates/fin-cli/tests/golden/tools.json`](crates/fin-cli/tests/golden/tools.json) and [`crates/fin-api/tests/golden/contract.json`](crates/fin-api/tests/golden/contract.json) as contract snapshots.
- Treat [`docs/benchmarks/generated/`](docs/benchmarks/generated/) as generated benchmark evidence that is committed on purpose.

## 3. Stack

| Layer | Choice | Notes |
| --- | --- | --- |
| Core runtime | Rust 2024 workspace + `rusqlite` | `bundled` SQLite, strict workspace linting, no `unsafe` |
| Agent surfaces | `clap`, `axum`, `ratatui` | CLI, read-only local daemon, terminal UI |
| Web | Bun workspace + SvelteKit 2 + Svelte 5 + Tailwind 4 + `svelte-adapter-bun` | SSR dashboard; server loaders call `fin-api` |
| Charts | `echarts` | Sankey, treemap, line, projection, grouped account visuals |
| Validation | Cargo tests/clippy/fmt + Bun tests + `svelte-check-rs` | root quality gate is `bun run util:check` |
| Bench/fixtures | deterministic fixture generator + `hyperfine` + `curl` | perf docs are part of the repo workflow |

## 4. Commands

- `bun install` - install Bun workspace deps and Husky hooks from [`bun.lock`](bun.lock)
- `cargo run -p fin-cli -- tools` - inspect the live CLI/tool contract before changing command metadata
- `cargo run -p fin-api -- start --check-runtime` - start the read-only daemon and fail fast on runtime issues
- `cargo run -p fin-tui --` - launch the Ratatui dashboard locally
- `bun run dev` - start `fin-api` plus the web dev server; defaults to Unix socket orchestration and switches to TCP when `FIN_API_BASE_URL` or `FIN_API_TRANSPORT=tcp` is set
- `bun run util:fixtures:generate` - materialize the synthetic runtime in `target/bench-fixtures/benchmark-runtime`
- `bun run util:bench:baseline` - regenerate CLI/SDK baseline artifacts under [`docs/benchmarks/generated/`](docs/benchmarks/generated/)
- `bun run util:bench:api` - benchmark `fin-api` over a Unix socket and refresh committed CSV/JSON evidence
- `bun run util:check` - full repo gate; note that it ends by running `bun run build`

## 5. Architecture

- [`crates/fin-sdk/src/`](crates/fin-sdk/src/): source of truth for config loading, rules, DB schema/migrations, imports, sanitization, mutations, read models, projections, and benchmark fixtures.
- [`crates/fin-cli/src/`](crates/fin-cli/src/): thin command adapters plus envelope/error handling. Public command metadata comes from [`crates/fin-sdk/src/contracts.rs`](crates/fin-sdk/src/contracts.rs); keep registry and real payloads in sync.
- [`crates/fin-api/src/api.rs`](crates/fin-api/src/api.rs): read-only HTTP JSON surface over SDK read models. `/__probe` is the probe exception; everything else follows the envelope contract and exposes config, rules, sanitize discovery, views, reports, and dashboard endpoints.
- [`crates/fin-tui/src/fetch/loaders.rs`](crates/fin-tui/src/fetch/loaders.rs): reads directly from SDK runtime/context. Cache keys include route plus context; do not collapse them to route-only keys.
- [`packages/web/src/lib/server/`](packages/web/src/lib/server/): server-only transport and mapping layer for `fin-api`. The web package must not query SQLite directly and must not recreate business logic already present in Rust.
- Contract invariant: non-interactive CLI commands emit one JSON envelope to stdout by default; `--text` opts into human-readable output; logs and diagnostics belong on stderr; `fin start` and `tui.start` are interactive-only.
- Surface nuance: the repo now supports `report burn`, reserve modes, and two-pool runway scenarios. If you change those flows, update SDK contracts, CLI adapters, API handlers, tests, and docs together.

## 6. Runtime and State

- FIN home precedence: `FIN_HOME` -> `TOOLS_HOME/fin` -> `$HOME/.tools/fin`
- Config precedence: explicit path -> `FIN_CONFIG_PATH` -> `$FIN_HOME/data/fin.config.toml`
- DB precedence: CLI `--db` or explicit SDK/API path -> `DB_PATH` -> `<config_dir>/fin.db` -> `$FIN_HOME/data/fin.db`
- Rules precedence: explicit path -> `FIN_RULES_PATH` -> `sanitization.rules` from config -> `$FIN_HOME/data/fin.rules.json`
- Runtime directories: `$FIN_HOME/data/{fin.config.toml,fin.rules.json,fin.rules.ts,fin.db,backups/}`, `$FIN_HOME/imports/{inbox,archive}`, `$FIN_HOME/run/fin-api.sock`
- The repo root commonly contains ignored local runtime directories [`data/`](data/) and [`imports/`](imports/) plus generated [`target/`](target/) and [`node_modules/`](node_modules/). Do not treat them as canonical source.
- Generated artifacts: [`packages/web/build/`](packages/web/build/), [`packages/web/.svelte-kit/`](packages/web/.svelte-kit/), and `target/bench-fixtures/*` should be regenerated, not edited by hand.
- Committed generated artifacts: [`docs/benchmarks/generated/`](docs/benchmarks/generated/) and the CLI/API golden JSON files are refreshed by scripts or tests and should not be hand-edited.
- Important side effect: [`package.json`](package.json) makes `bun run build` copy `fin`, `fin-tui`, and `fin-api` into `${FIN_HOME:-${TOOLS_HOME:-$HOME/.tools}/fin}`. `bun run util:check` triggers that install step too.

## 7. Conventions

- Use `fin tools` and `GET /v1/tools` as live contract discovery before changing command or endpoint metadata.
- Preserve the envelope shape `ok`, `data | error`, `meta` across CLI and API surfaces. Probe responses are intentionally outside that contract.
- Web server queries use camelCase URL params such as `downsampleMinStepDays`, `minimumBurnRatio`, `sortField`, and `sortDirection`; keep API parsing and web tests aligned.
- The web package is designed to degrade cleanly when `fin-api` is unavailable; preserve the fallback/skeleton behavior in [`packages/web/src/lib/server/skeleton.ts`](packages/web/src/lib/server/skeleton.ts) and related tests.
- Golden updates are opt-in through `UPDATE_GOLDEN=1` in the CLI and API contract tests. Only rewrite snapshots when contract drift is intentional and reviewed.

## 8. Constraints

- Never commit personal runtime data, imported statements, or home-directory rules. Keep committed examples and fixtures sanitized.
- Never add direct write behavior to `fin-api` or direct SQLite access to the web package without an explicit architecture change.
- Use `--dry-run` first for `sanitize migrate`, `sanitize recategorize`, `view void`, and `edit transaction`.
- Treat [`crates/fin-sdk/src/db/schema.rs`](crates/fin-sdk/src/db/schema.rs), [`crates/fin-sdk/src/db/migrate.rs`](crates/fin-sdk/src/db/migrate.rs), [`crates/fin-sdk/src/import.rs`](crates/fin-sdk/src/import.rs), and [`crates/fin-api/src/api.rs`](crates/fin-api/src/api.rs) as high-risk contract areas.
- Do not conflate benchmark numbers. The repo explicitly separates handler latency, end-to-end request time, and broader QA/SSR timing in the benchmark docs.
- If you need isolated validation, point `FIN_HOME` at a fixture runtime before running the root build/check scripts so you do not overwrite the default installed tool directory.

## 9. Validation

- Required gate: `bun run util:check`
- Rust coverage: `cargo test --workspace`
- Web-only changes: `bun run --filter @fin/web check` and `bun run --filter @fin/web test`
- Contract changes: `cargo test -p fin-cli --test contract_parity` and `cargo test -p fin-api --test contract_parity`
- Intentional contract snapshot changes: rerun the same tests with `UPDATE_GOLDEN=1` and commit the resulting golden JSON updates
- Fixture changes: `cargo test -p fin-sdk --test fixture_determinism`
- Runtime smoke checks for operator-facing changes: `cargo run -p fin-cli -- tools`, `cargo run -p fin-api -- start --check-runtime`, and the relevant `bun run dev` or `cargo run -p fin-tui --` path
- If you touch benchmarked SDK/API paths, rerun `bun run util:bench:baseline` or `bun run util:bench:api` and refresh the matching docs under [`docs/benchmarks/`](docs/benchmarks/)

## 10. Further Reading

- [`PLAN.md`](PLAN.md) - roadmap operating rules and acceptance criteria
- [`tests/parity/archive-parity-matrix.csv`](tests/parity/archive-parity-matrix.csv) - archive-to-main surface parity checklist for dashboard, overview, and transactions work
