# pr-reviewer — handoff / context

A strict, **diff-scoped pre-PR code-review subagent**. It runs **internally, before
a PR is opened**, so the external reviewer (CodeRabbit) finds fewer bugs/issues.
It **complements** — does not replace — CodeRabbit and the domain critics. This
file is the reasoning behind the design so a fresh session doesn't undo it.

## Role (clarified)

The repo already has: domain authors + paired critics, a Stop review-gate hook
(`.claude/hooks/review-gate.mjs` + `review-routes.json`), CodeRabbit
(`.coderabbit.yaml`), and `/review-*` domain commands. Those critics are
**domain-scoped and LLM-only** (they don't run tools or trace blast radius), so
cross-cutting/correctness bugs slip past them — exactly the class CodeRabbit kept
catching post-PR. `pr-reviewer` closes that gap as the **final internal gate before
push**: a generalist, tool-running, blast-radius, verification-gated reviewer.

## Files

- `.claude/agents/pr-reviewer.md` — the reviewer subagent (Bash/Read/Grep/Glob, **no
  Write** — advisory). Model `opus` for max defect recall.
- `.claude/commands/review.md` — `/review [base-ref]`, delegates to the subagent.
- `.claude/review-config.md` — path-specific rules + **learnings** (repo exceptions
  that keep the review from raising known false positives).

## How to run

- `/review` (or `/review main`) on demand, or `@pr-reviewer`.
- **In the per-PR pipeline**: it runs as the final internal gate AFTER the domain
  critics + cleanup and BEFORE push/PR. **🔴 and 🟠 must be resolved before the PR
  goes up**; 🟡/⚪ are advisory.
- Optional **husky** pre-push hook (the repo uses husky, not lefthook) can run it
  headless — deferred until we want auto-firing.

## Why it's built this way (don't strip these — they're what make it out-catch a stock reviewer)

1. **Runs the repo's own tools** and folds results in: `pnpm typecheck`
   (`tsc --noEmit`), `pnpm lint:strict` (eslint, esp. `react-hooks/exhaustive-deps`),
   `cargo clippy -D warnings`/`fmt --check`/targeted `cargo test`, `gen:ipc:check`,
   a secret scan, and tests covering touched files. This is the "40+ linters" layer
   pointed at OUR actual config. Don't replace tool output with the model re-deriving it.
2. **Cross-file blast-radius pass**: greps every changed/removed/renamed symbol's
   callers and reports defects _outside_ the diff (the CodeGraph idea). codegraph
   (`codegraph callers/impact`) is available and preferred for this.
3. **Verification gate**: each finding must be substantiated (construct the triggering
   input / trace the path) or dropped or marked ⚠️ Suspected. Keeps strict from
   becoming noisy.
4. **Stack-tuned invariant checks** (React 19 / TS / Tauri 2 + Rust):
   - Tauri: IPC is the trust boundary — flag `#[tauri::command]`s without input
     validation, fs/path commands without scope checks (path traversal), widened
     capability scopes, loosened CSP, validation done in the frontend instead of Rust.
   - React 19: wrong/missing effect deps, stale closures (fix with functional
     `setState`/ref/`useEffectEvent`, not lint suppression), missing cleanup → leaks,
     async-effect races without `AbortController`, hydration mismatches, missing keys,
     referential instability into memo/deps.
5. **Severity model** 🔴/🟠/🟡/⚪ + final verdict; stays inside the change's blast
   radius (pre-existing issues out of scope unless the change endangers them).

## Repo integration decisions (this repo)

- Registered as a **no-author cross-cutting critic** (like `tauri-security-reviewer`/
  `performance-profiler`): listed in CLAUDE.md + carded in `landing/agent-system.html`
  so `pnpm check:agent-system` (the drift guard) stays green. **Not** in
  `review-routes.json` (it's a generalist, not path-routed) and **not** in the
  author/critic PAIRS.
- **CodeRabbit stays** — `pr-reviewer` reduces what it finds, doesn't replace it.
- Enforcement: **🔴 + 🟠 block** the PR; 🟡/⚪ advisory.

## Open / next steps

- Fill `.claude/review-config.md` learnings as false positives show up.
- Wire the optional husky pre-push hook if we want it to fire automatically.
- Tune which tests Phase 1 runs (currently: tests covering touched files, not the
  full suite) and whether the hook blocks on 🔴 vs. stays advisory.
- Subagent edits load at session start — restart or re-create via `/agents` after
  editing `pr-reviewer.md`.
