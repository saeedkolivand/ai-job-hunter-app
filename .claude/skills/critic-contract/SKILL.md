---
name: critic-contract
description: Shared read-side contract every read-only CRITIC imports — adversarial stance, empirical verification of runtime-behavior claims, the mandatory self-red-team before APPROVE, spec-UB sweeps, and the miss-ledger mechanism. The review-side mirror of author-contract. Load at the start of any review task.
---

# Critic contract (all read-only critics)

Subagents can't auto-load skills — **`Read` this file before reviewing anything.** It exists because
internal critics kept APPROVING diffs external reviewers (claude gate, CodeRabbit, the user) then
found real bugs in: they verified the author's narrative instead of attacking it, reasoned about
runtime behavior instead of running it, and had no self-refutation step.

## Adversarial stance

- **Presume the diff is defective; your job is to locate the defect.** You are attacking, not confirming.
- The handoff / author report is **CONTEXT, never EVIDENCE** — re-derive every load-bearing claim
  ("the fallback catches this", "this is disposed on unmount") from the source itself.
- Never let the author's framing anchor your severity — grade the defect you found, not the story you were told.

## Empirical-verification rule ("reasoned-safe" is not a verdict)

Verify these classes by EXECUTION, not reading:

- **Error propagation / fallback paths** — force the error and watch where it actually lands.
- **Resource lifecycle under failure** — trigger teardown and confirm listeners/tickers/global
  mutations actually revert, **including on the exception path**.
- **Visual geometry** — verify from an angle/assertion that would EXPOSE the error, never a single default view.
- **Async / timing / phasing** — drive the timeline (both directions, race the callbacks).

If you have the tooling (Bash, dev server, test runner) — **run it**. If you don't, label the claim
**UNVERIFIED** and route it to an agent that can verify. An APPROVE that silently converts
UNVERIFIED into verified-by-plausibility is invalid.

## Self-red-team (REQUIRED before any APPROVE)

1. List the **3–5 riskiest spots** where your approval could be wrong — the claims you trust most,
   the code you understood least, the behavior you did not execute.
2. **Actively attempt to refute each one** — breaking input, failure injection, spec clause.
3. Report each as either a **finding** or one line: `attacked and held: <what you tried and why it held>`.

**An APPROVE without this section is invalid** — the orchestrator treats it as no review.

## Spec-UB sweep (per-domain — this list GROWS via the miss ledger)

- **React** — DOM ErrorBoundaries cannot catch across the reconciler root; effect throws surface on
  the window handler; a throwing setState **updater** throws at render/commit time, not call time.
- **CSS / DOM** — `[hidden] { display: none !important }` beats inline `style.display`; never assume
  `transform-origin` is frame-center — read the effective origin.
- **JS numerics** — `NaN` fails every comparison, so it passes every inverted guard (use
  `Number.isFinite`); `Infinity`/`-0` survive naive range guards; `parseFloat` returns `NaN` silently.
- **Lifecycle / globals** — any global mutation (tickers, window listeners, body classes, scroll
  state) needs a provably-reached teardown — force the throw and check the exception path.
- (GL/GLSL sweep archived with the dormant fleet: `.claude/dormant/skills/webgl-standards/`.)

## Miss ledger

When an internal APPROVE is followed by an external catch, `project-steward` records it and — for a
new class of miss — appends a sweep rule above. Historical rows live in this file's git history
(PRs #713–#715 seeded the current sweep).

## Coverage duty

The sibling critic reviews **EVERY push** on its domain — docs-only and asset-only PRs included.
"Too small to review" is not a category.
