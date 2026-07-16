# ADR-025: Agent fleet — paired author/critic per domain

Last updated: 2026-06-19

**Status:** Accepted

## Context

The `.claude/` agent fleet was audit-biased: most agents were read-only reviewers, so when the main session delegated _implementation_ it fell to generic `general-purpose` agents that lacked the domain reviewers' grounding. There was no live mechanism for agents to correct each other or share working context, and the agent definitions / docs / parallel AI-tool config files (aider, cursor, copilot, windsurf, cline, codex, roo, jba, AGENTS.md) had drifted (some still described the removed Electron app).

## Decision

Every domain is a **pair**: a write-capable **author** implements, an independent **critic** audits — the author never approves its own work (intrinsic self-correction is unreliable; the critic must be a different agent). Added 5 authors (`rust-backend-author`, `frontend-author`, `job-match-author`, `ai-provider-author`, `scraping-applier-author`), kept `pdf-docx-generator`/`test-author`/`code-quality-author` as authors, and added `ui-ux-expert` (visual/UX critic). Context flows through a per-task handoff file (`.claude/scratch/<task>.md`); coordination is sequential subagents by default, native Agent Teams (behind `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS`, in-process on Windows) only when parallelism genuinely pays. A release bug-gate (`/prepare-release` pre-flight) catches bugs before release. CLAUDE.md + `.claude/` is the single source of truth; the other AI-tool configs are thin pointers to it. A deterministic guard (`scripts/check-agent-system.mjs`, wired into `.husky/pre-push` + CI) keeps agents ⇄ routes ⇄ CLAUDE.md ⇄ docs ⇄ AI-configs in sync.

## Consequences

- **25 agents:** the routing table + explainer must list every one (the guard enforces it).
- **Token premium of pairing/teams:** offset by the pre-harvest handoff (no cold re-exploration).
- **Agent Teams experimental:** Windows/VS Code runs in-process only (no tmux split panes).
- **Visual explainer:** `landing/agent-system.html` documents the system.
- **Guard enforcement:** `scripts/check-agent-system.mjs` runs in pre-push hook and CI to keep agent definitions, routes, and configs in sync.

## Related

- `.claude/agents/` — 25 agent definitions (author + critic per domain)
- `.claude/commands/` — 15 commands (implement-feature, fix-bug, refactor-module, add-tests, review-\* specialties, prepare-release, etc.)
- `.claude/review-routes.json` — deterministic routing from touched files to primary + risk-justified secondary reviewers
- `CLAUDE.md` — single source of truth for agent descriptions and routing
- `scripts/check-agent-system.mjs` — deterministic guard (pre-push + CI) enforcing agents ⇄ routes ⇄ CLAUDE.md ⇄ AI-configs
- `landing/agent-system.html` — interactive visual explainer of the 25-agent system
