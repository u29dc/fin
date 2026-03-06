# fin Rewrite Execution Plan

## Purpose

This file is the operator brief for the roadmap in `tickets.csv`.

Use `tickets.csv` as the source of truth for ticket-by-ticket execution.
Use this document for shared rules, sequencing, constraints, and acceptance notes that would be repetitive or awkward inside every CSV row.

## Artifacts

- `tickets.csv`: machine-friendly backlog with sequence, dependencies, status, success criteria, validation, instructions, and watch-outs.
- `PLAN.md`: shared operating rules and domain decisions for the roadmap.
- `docs/fin-api.md`: operator-facing daemon usage and troubleshooting notes.
- `docs/benchmarks/api-007-fin-api.md`: recorded fin-api benchmark interpretation and artifact links.
- `docs/benchmarks/qa-002-final.md`: final end-to-end validation record for the completed roadmap.

## Status Rules

Use these status values only:

- `done`: completed and validated against the ticket's validation field.
- `ready`: actionable now; no unmet dependencies remain.
- `blocked`: not yet actionable because one or more dependencies are not done.
- `in_progress`: optional working state while executing exactly one ticket.
- `failed`: attempted and blocked by a real unresolved issue that must be documented before moving on.

When multiple tickets become `ready`, pick the lowest `sequence` first.
Do not start a higher-sequence ticket while a lower-sequence ready ticket remains untouched unless there is a documented reason in the ticket file or commit history.

## Ticket Update Rules

After each ticket:

1. Update that ticket's `status` in `tickets.csv`.
2. Update any dependent tickets that are now unblocked from `blocked` to `ready`.
3. Add or refine dependencies only if new code facts require it.
4. Keep IDs stable. Do not rename existing ticket IDs once work has started.
5. Keep one ticket scoped to one coherent deliverable. Split only if a ticket proves too large in practice.

## Non-Negotiable Architecture Rules

- Rust SDK remains the single source of domain and read-model truth on `main`.
- `fin-api` is a thin Rust daemon over the SDK, not a second core.
- Restored web package must call `fin-api`; it must not query SQLite directly and must not revive the archived TypeScript or Bun core as a runtime dependency.
- TUI must consume shared SDK read models, not re-aggregate business logic in route render code.
- JSON envelope shape remains `ok`, `data` or `error`, and `meta`.
- Read-only API scope must exclude mutating and interactive-only commands.

## Transport Decision

Default local transport for `fin-api`:

- HTTP plus JSON over Unix domain socket.

Fallback transport:

- loopback TCP only when required for portability or debugging.

Rationale:

- local-only security by default
- no port collision risk
- Bun server-side loaders can talk to Unix sockets directly
- Axum and Tokio support the transport cleanly

Do not invent a custom binary IPC protocol unless HTTP over a Unix socket is proven too slow by measurement.

## Performance Targets

These are final targets, not assumptions of current state.
QA-001 must record the baseline and QA-002 must record the final measured state.

Measure and record these separately:

- SQL or storage time where useful for diagnosis
- end-to-end API handler time
- full SSR page time
- TUI route fetch time if explicitly benchmarked

Target budgets:

- Warm local API transaction list for a page containing thousands of rows: aim for sub-5 ms handler latency on the benchmark fixture.
- Warm local API summary or dashboard endpoints: keep them as low-latency as practical and record exact numbers; do not hide repeated scans behind wishful thinking.
- Web SSR route rendering should be measured separately from API latency and should not be reported as if it were the same metric.

If sub-5 ms is not met for the transactions path, PERF-003 is not done.
If a target cannot be met, document the exact blocker and measured evidence rather than weakening the ticket silently.

## Shared Validation Rules

Before marking any ticket `done`:

- Run the validation commands or checks listed in the ticket.
- Verify any public contract changes with snapshots or direct command output where appropriate.
- For performance tickets, record the exact benchmark command and dataset.
- For TUI tickets, include manual terminal validation at more than one width.
- For web tickets, include `check` and `build` plus manual runtime validation where needed.

## Common Watch-Outs

- Partial-month financial data can corrupt cashflow and KPI semantics if included by accident.
- Income and expense sign conventions must stay explicit at all boundaries.
- `readonly` plus `migrate` database behavior is subtle; treat startup and migration semantics carefully in shared read code and fin-api.
- Route-level TUI caching must include context, not just the route enum.
- Pagination must use stable tie-breaks such as timestamp plus unique ID.
- Do not let restored web charts recompute business logic that already exists in Rust.
- Do not use personal runtime data from `$FIN_HOME` in committed fixtures or tests.
- Do not claim performance wins without recorded before and after commands.

## Archive Parity Surfaces To Preserve

These archive capabilities are specifically worth restoring:

- dense per-group KPI strip
- asset allocation and reserve composition
- monthly cashflow comparison with anomaly cues
- flow-of-funds distribution
- hierarchical expense breakdown
- historical all-account balance context
- forward runway projection with thresholds
- scalable grouped and sortable transactions view

TUI does not need to imitate the exact visual chart types.
It does need to recover the same decision value.

## TUI Visual Rules

- Prefer grouped monthly bars, segmented bars, trees, matrices, and sparklines over faux line charts.
- Use color as reinforcement, not as the only carrier of meaning.
- Keep labels explicit, numeric text tabular, and sign conventions visible.
- Favor dense, calm layouts over decorative borders or empty space.
- Design for common terminal widths first, then add richer behavior for wide terminals.

## Definition Of Roadmap Done

The roadmap is done only when all of the following are true:

- `tickets.csv` reflects real ticket completion states.
- richer Rust SDK read models exist for the archive-derived surfaces that matter
- TUI is materially more informative than the current main-branch baseline
- `fin-api` exists and exposes all intended read-only functionality
- `packages/web` is back on `main` and uses `fin-api`, not the archived core
- `bun run dev` starts the full stack cleanly
- quality gates, benchmarks, and manual validation are recorded and passing or explicitly documented

Final completion evidence is recorded in `docs/benchmarks/qa-002-final.md`.
