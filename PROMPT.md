# Autonomous Agent Instructions

You are an autonomous coding agent working through a structured PRD.

## Context

You are working through a structured PRD for the `fin` CLI. The stories in `prd.json` define all
work to be done -- follow the data, not assumptions about what is or isn't complete.

The codebase is a monorepo: `packages/core/` (backend logic, SQLite), `packages/cli/` (agent-native
toolbelt), `packages/web/` (SvelteKit frontend). Stories may touch any package.

Reference materials (use when stories reference them):
- `.tmp/AGENT_NATIVE.md` -- agent-native architecture specification
- `.tmp/let/packages/cli/src/` -- proven reference implementation (envelope.ts, tool.ts, commands/)

Key patterns:
- `envelope.ts`: ok()/fail()/isJsonMode()/emitRaw() for JSON output
- `tool.ts`: defineToolCommand() wraps citty and auto-registers in toolRegistry[]
- Each command: defineToolCommand() + isJsonMode() branching + ok()/fail() for JSON, existing renderers for text
- `tools` and `health` use plain defineCommand() (infrastructure, not tools)

## Phase 0: Orient

1. Study `prd.json` in the project root
2. Study `progress.txt` -- read the **Codebase Patterns** section FIRST
3. Run `git log --oneline -20` to understand recent changes
4. Identify the next story: highest priority where `passes: false`
5. If the story has `dependsOn`, verify all dependencies have `passes: true`; if not, skip to next eligible story
6. Output the story you are working on: <ralph-status>{STORY_ID}</ralph-status>

## Phase 1: Understand Before Acting

- Study codebase files relevant to your story using search tools
- Do NOT assume functionality is missing; confirm with code search first
- Check `.tmp/let/packages/cli/src/` for reference patterns matching your story
- Check `.tmp/AGENT_NATIVE.md` for the exact specification of the command you are implementing
- Read existing `packages/cli/src/commands/*.ts` to understand current text output patterns before migrating
- Read `packages/core/src/` exports to find the exact query functions you need

## Phase 2: Implement

- Implement the single user story you selected
- Follow existing code patterns and conventions
- Keep changes focused and minimal
- Only modify files necessary for this story
- Every command must support both `--json` (envelope) and text (existing renderers) modes

## Phase 3: Verify

Run the quality gate:

```
bun run util:check
```

This MUST pass before committing. If it fails, fix and re-run until green. Do NOT skip or weaken quality checks. Never edit tests just to make them pass -- fix the implementation instead. You may edit tests to adapt to legitimate code changes or to improve coverage.

## Phase 4: Commit and Update

### Commit Format (MANDATORY)

```
type(scope): subject line

- Bullet explaining why this change was made
- Another bullet if needed
```

Rules:
- Types: feat, fix, refactor, docs, style, chore, test
- Scopes: web, core, cli, db, import, config, deps, docs, ci
- Subject: lowercase, no trailing period, max 100 chars
- Body: required, bullet points explaining "why", max 100 chars per line

### Steps

1. Stage all changed files: `git add -A`
2. Commit with the exact format above
3. Update `prd.json`: set `passes: true` for the completed story
4. Append progress report to `progress.txt` (never replace existing content):

```
## {DATE} - {STORY_ID}: {STORY_TITLE}
- What was implemented
- Files changed
- **Learnings:** patterns discovered, gotchas, useful context
---
```

5. If you discover a reusable pattern, add it to the **Codebase Patterns** section at the TOP of progress.txt

## Stop Condition

After completing the story, check if ALL stories in prd.json have `passes: true`.

If ALL complete: <promise>COMPLETE</promise>

If stories remain: end your response normally.

## 999. Critical Rules

- Work on ONE story per iteration
- NEVER commit code that fails the quality gate
- NEVER edit tests as a shortcut to get green -- fix the code, not the tests
- NEVER assume functionality is missing without searching first
- ALWAYS read Codebase Patterns before starting
- ALWAYS use the exact commit format specified above
- ALWAYS check .tmp/AGENT_NATIVE.md for command specifications
- ALWAYS check .tmp/let/ for reference implementation patterns
- Keep changes focused -- do not refactor unrelated code
- Do not add features not specified in the current story
