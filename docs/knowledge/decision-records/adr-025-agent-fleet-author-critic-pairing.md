# ADR-025: Agent fleet — paired author/critic per domain

Last updated: 2026-07-28

**Status:** Accepted

## Context

The `.claude/` agent fleet was audit-biased: most agents were read-only reviewers, so when the main session delegated _implementation_ it fell to generic `general-purpose` agents that lacked the domain reviewers' grounding. There was no live mechanism for agents to correct each other or share working context, and the agent definitions / docs / parallel AI-tool config files (aider, cursor, copilot, windsurf, cline, codex, roo, jba, AGENTS.md) had drifted (some still described the removed Electron app).

## Decision

Every domain is a **pair**: a write-capable **author** implements, an independent **critic** audits — the author never approves its own work (intrinsic self-correction is unreliable; the critic must be a different agent). Added 5 authors (`rust-backend-author`, `frontend-author`, `job-match-author`, `ai-provider-author`, `scraping-applier-author`), kept `pdf-docx-generator`/`test-author`/`code-quality-author` as authors, and added `ui-ux-expert` (visual/UX critic). Context flows through a per-task handoff file (`.claude/scratch/<task>.md`); coordination is sequential subagents by default, native Agent Teams (behind `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS`, in-process on Windows) only when parallelism genuinely pays. A release bug-gate (`/prepare-release` pre-flight) catches bugs before release. CLAUDE.md + `.claude/` is the single source of truth; the other AI-tool configs are thin pointers to it. A deterministic guard (`scripts/check-agent-system.mjs`, wired into `.husky/pre-push` + CI) keeps agents ⇄ routes ⇄ CLAUDE.md ⇄ docs ⇄ AI-configs in sync.

## Consequences

- **Agent roster:** the routing table + explainer must list every one in `.claude/agents/` (the guard enforces it).
- **Token premium of pairing/teams:** offset by the pre-harvest handoff (no cold re-exploration).
- **Agent Teams experimental:** Windows/VS Code runs in-process only (no tmux split panes).
- **Visual explainer:** `apps/landing/src/app/agent-system/` documents the system (data roster: `apps/landing/src/data/agent-fleet.ts`).
- **Guard enforcement:** `scripts/check-agent-system.mjs` runs in pre-push hook and CI to keep agent definitions, routes, and configs in sync.

## Amendment: cost-tiered per-change defaults (2026-07-28)

The initial design routed **every change** through the agent fleet (author + sibling critic minimum, full trio on risk-bearing work). Practice over 6.5 months revealed the per-finish LLM review in the Stop hook was the setup's largest recurring token cost: 830 stop-gate runs over 17 days, 74.7% timeout failures (fail-open budget), 68 findings with only 8 critical blocks ever shipped.

**New model:** per-change cost tiering scales review depth to actual risk.

- **Trivial diffs** (docs, config, one-liners, renames): main session direct edit (no agent), Stop gate review-only (deterministic AST + ledger re-emits).
- **Single-domain non-risk** (e.g., UI component, single Rust module): ONE sibling critic (author + critic pair); testable logic still gets the mandatory test pair (`test-author` → `testing-reviewer`).
- **Full trio** (risk-bearing, multi-domain, security, breaking changes): author + both critics.
- **Pre-PR security gate** (`/review-security`): always tauri-security-reviewer (Opus xhigh).
- **Pre-PR logic gate** (`/review`): always pr-reviewer (Opus xhigh) as the final fence before merge.

**Telemetry shift:** usage/cost metrics (scripts/pre-push-review.mjs) exist **before** a review surface ships, not after. Pre-push MCP boot-timeout is hardcoded (fail-close on timeout, never retry), so stuck pushes are detectable as MCP-server metrics rather than silent stalls.

**Consequence:** the Stop hook's per-finish LLM review is retired; review depth is author-side determinism + risk-based critic assignment + pre-push/pre-PR explicit gates. Full rigor remains on demand via `/review` + `/review-security` + CI + CodeRabbit.

## Related

- `.claude/agents/` — agent definitions (author + critic per domain; roster enforced by `scripts/check-agent-system.mjs`)
- `.claude/commands/` — slash commands (implement-feature, fix-bug, refactor-module, add-tests, review-\* specialties, prepare-release, etc.)
- `.claude/review-routes.json` — deterministic routing from touched files to primary + risk-justified secondary reviewers
- `CLAUDE.md` — single source of truth for agent descriptions and routing
- `scripts/check-agent-system.mjs` — deterministic guard (pre-push + CI) enforcing agents ⇄ routes ⇄ CLAUDE.md ⇄ AI-configs
- `apps/landing/src/app/agent-system/` — interactive visual explainer (data roster: `apps/landing/src/data/agent-fleet.ts`)
