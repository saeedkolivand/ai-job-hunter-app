@AGENTS.md

<!--
  Portable rules live in AGENTS.md (canonical; every tool reads it). This file adds ONLY
  Claude-Code-specific orchestration: the subagent fleet, review gates, model tiering.
  Nothing here is portable: Cursor cannot spawn `rust-backend-architect`.
  Keep the loaded content under ~60 lines; HTML comments like this one are stripped before injection.
-->

## Claude Code

### Auto-invoked skills (always on, no slash command)

Re-injected every session by the `SessionStart` hook (`.claude/hooks/style-policy.mjs`).

- **`ponytail`**: lazy-senior-dev default: laziest solution that works (YAGNI, stdlib/native over deps). Intensity `full`; off: `stop ponytail` / `normal mode`. Source: `ponytail@ponytail` plugin.
- **`grill-with-docs`**: before finalizing any non-trivial plan/design (incl. before `ExitPlanMode`), stress-test it against the domain model + ADRs. Skip for trivial/one-line/docs changes. Source: the user-level `grill-with-docs` skill.

### Agent system

Domain pairs: a write-capable **author** implements, an independent **critic** audits (authors never approve their own work). Full pipeline detail → `.claude/` (agents/skills/commands), routing → `.claude/review-routes.json`, knowledge base → `docs/knowledge/`. Drift guard: `pnpm check:agent-system` (pre-push + CI).

| Touched area                                                | Author                          | Critic(s)                                        |
| ----------------------------------------------------------- | ------------------------------- | ------------------------------------------------ |
| React renderer                                              | `frontend-author`               | `frontend-reviewer` · `ui-ux-expert` (visual/UX) |
| Landing site (apps/landing, Next.js 16 static export)       | `frontend-author`               | `frontend-reviewer` · `ui-ux-expert` (visual)    |
| Rust/Tauri backend                                          | `rust-backend-author`           | `rust-backend-architect`                         |
| Resume/export, DocumentModel, templates, theme, locale      | `pdf-docx-generator`            | `resume-export-expert`                           |
| ATS scoring, job analysis, matching, cover-letter relevance | `job-match-author`              | `job-match-expert`                               |
| AI providers, routing, embeddings, prompts, streaming       | `ai-provider-author`            | `ai-provider-expert`                             |
| Scraping, browser automation, registries                    | `scraping-applier-author`       | `scraping-applier-expert`                        |
| Browser extension + bridge + protocol                       | `extension-author`              | `extension-reviewer`                             |
| Tests                                                       | `test-author`                   | `testing-reviewer`                               |
| Code quality (on-demand)                                    | `code-quality-author`           | `code-quality-reviewer`                          |
| Docs / knowledge / ADRs / lessons / release                 | `project-steward` (sole writer) | `project-steward`                                |

Cross-cutting critics (no author; fixes route to the owning domain author): `tauri-security-reviewer` (Secondary on risk-bearing changes), `performance-profiler` (perf-sensitive only), `cleanup` (dead-code, pre-PR), `pr-reviewer` (strict pre-PR gate), `finding-verifier` (per-finding judge for /review, ≤5 spawns). The dormant GL fleet + skills live in `.claude/dormant/` (no GL surface since ADR-0017). Every critic loads `.claude/skills/critic-contract/SKILL.md`; every author loads `author-contract`.

**Per-change defaults (cost-tiered, ADR-025):**

- **Trivial diffs** (docs/config/comments/renames/single-file ≤10 lines): the **main session edits directly**, no swarm. The deterministic Tier-0 Stop gate covers them; LLM review happens at pre-PR/CI for higher-risk changes.
- **Single-domain, non-risk changes**: author → **ONE sibling critic** (resolve HIGH/CRITICAL; LOW/MEDIUM advisory) → if testable logic, `test-author` → `testing-reviewer`.
- **Risk-bearing** (any `tauri-security-reviewer` secondary glob) **or multi-domain**: full trio incl. security critic (≤3 critics/task).
- `project-steward` closes **once per PR** (docs/lessons sync + `graphify update .`), not once per change. Context flows via `.claude/scratch/<task>.md`. Orchestrate all sub-agents from the main session (agents can't call agents).
- **Before a PR:** `/review-security` (HIGH/CRITICAL block) then `/review` (🔴+🟠 block); both complement CodeRabbit.

**Review gates:** the Stop hook (`.claude/hooks/review-gate.mjs`) is **deterministic-only** (ast-grep Tier-0 + ledger re-emits, near-free, blocks on introduced arch violations and unresolved findings). **LLM review happens pre-PR via the agent internal chain + CodeRabbit + CI** (`🤖 AI Review OK` required check); metrics in `.claude/.review-metrics.jsonl` include token usage + cost.

**Hard rules:**

- Non-trivial source changes go through a domain author via `Agent`; the main session edits directly only for trivial diffs (above), the rule files (`AGENTS.md`/`CLAUDE.md`), `.claude/**` meta-config, and plan files.
- **Lessons** (`.claude/memory/lessons.jsonl`): only `project-steward` writes; others propose via `LESSON · category · Context/Decision/Outcome`.
- **Cross-session recall**: agents may call `mcp__mcp-search` (claude-mem plugin; absent if not installed). Honor `docs/` path-privacy + `<private>` for PII.

**Model & effort tiering** (per-agent frontmatter; aliases track the current family): Opus for last-line critics: `pr-reviewer` + `tauri-security-reviewer` pin `effort: xhigh`; the other Opus critics (`rust-backend-architect`, `ai-provider-expert`, `job-match-expert`) run `high`. Sonnet for authors + balanced critics (default `high`). Haiku for `project-steward` + `finding-verifier` (`low` fine for mechanical runs). Escalate a spawn to Opus for genuinely hard work; to Fable only for hard, ambiguous, cross-domain work; never as a frontmatter default, never for security review.

**Context priority:** codegraph/graphify → source → `docs/knowledge/` → lessons. Read the minimum; stop at ~90% confidence.
