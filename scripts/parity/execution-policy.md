# Unattended Execution Policy

This policy governs autonomous rewrite execution against `.tmp/PLAN.md`.

## 1. Checkpoint cadence

- Update `.tmp/PLAN.md` after every ticket and every stage.
- Re-read `.tmp/PLAN.md` after each stage before starting the next one.
- Record validation/evidence for every completed ticket.

## 2. Blocker classes

- Hard blocker:
  - Data loss risk
  - Contract ambiguity that can cause irreversible drift
  - Missing environment dependency required for parity-critical command
- Soft blocker:
  - Optional enhancement cannot be completed in-stage
  - Non-critical style drift

Hard blockers must stop stage progression until mitigated.

## 3. Archive safety rules

- Never commit rewrite code to `archive`.
- Never force-push or rebase `archive`.
- Treat `archive` as immutable behavioral reference.

## 4. Personal data safety

- Never commit files from `$FIN_HOME/data` or `$FIN_HOME/imports`.
- Use `PARITY_FIN_HOME` isolation for all fixture/certification scripts.
- Keep only sanitized fixtures/templates in repository.

## 5. Ticket completion criteria

A ticket is complete only when:

1. Implementation changes are in place.
2. Targeted validations were run.
3. Evidence was captured (logs/reports or deterministic outputs).
4. Atomic commit created with commitlint-compliant message.

## 6. Failure handling

- If validation fails, fix in the same ticket scope when possible.
- If fix requires cross-ticket scope, mark ticket blocked, document reason, then continue with independent tickets.
