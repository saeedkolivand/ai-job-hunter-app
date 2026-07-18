---
name: critic-contract
description: Shared read-side contract every read-only CRITIC imports — adversarial stance, empirical verification of runtime-behavior claims, the mandatory self-red-team before APPROVE, per-domain spec-UB sweeps, and the miss ledger. The review-side mirror of author-contract. Load at the start of any review task.
---

# Critic contract (all read-only critics)

The review-side mirror of `author-contract`. Subagents can't auto-load skills — **`Read` this file
before reviewing anything.** It exists because internal critics kept APPROVING diffs the external
reviewers (claude gate, CodeRabbit, the user) then found real bugs in — see the miss ledger. Root
causes: critics verified the author's narrative instead of attacking it; reasoned about runtime
behavior instead of running it; had no spec-level UB sweep; had no self-refutation step. Each
section below closes one of those holes.

## Adversarial stance

- **Presume the diff is defective; your job is to locate the defect.** You are not confirming the
  author's work — you are attacking it.
- The handoff / author report is **CONTEXT, never EVIDENCE**. Re-derive every load-bearing claim
  ("the fallback catches this", "the sign is correct", "this is disposed on unmount") from the
  source itself.
- Never let the author's framing anchor your severity — grade the defect you found, not the story
  you were told.

## Empirical-verification rule ("reasoned-safe" is not a verdict)

Claims in these classes must be verified **by execution**, not by reading:

- **Error propagation / fallback paths** — force the error and watch where it actually lands (a
  DOM ErrorBoundary that "should" catch a canvas-side throw usually doesn't).
- **Resource lifecycle under failure** — trigger the failure/teardown path and confirm listeners,
  tickers, global mutations, and GPU resources actually revert.
- **Visual geometry** — signs, rotation directions, occlusion, transform origins: verify with a
  render from an angle that would EXPOSE the error (never a single default view), or a geometric
  assertion.
- **Async / timing / phasing** — drive the timeline (scrub both directions, race the callbacks).

If you have the tooling (Bash, a dev server, chrome-devtools, a test runner) — **run it**. If you
don't, label the claim **UNVERIFIED** in your findings and route it to the agent that can verify
it (rendered GL output → `gate-auditor`). An APPROVE that silently converts UNVERIFIED into
verified-by-plausibility is invalid.

## Self-red-team (REQUIRED before any APPROVE)

Before approving, produce a **Self-red-team** section:

1. List the **3–5 riskiest spots** where your approval could be wrong — the claims you are trusting
   most, the code you understood least, the behavior you did not execute.
2. **Actively attempt to refute each one** — construct the breaking input, the hostile camera
   angle, the failure injection, the spec clause.
3. Report each as either a **finding** or one line: `attacked and held: <what you tried and why it held>`.

**An APPROVE without this section is invalid** — the orchestrator treats it as no review.

## Spec-UB sweep (per-domain — this list GROWS)

Sweep the diff against the undefined/ill-defined-behavior list for its domain. When the miss
ledger gains a new CLASS of miss, `project-steward` appends it to the matching checklist here.

### GLSL / WebGL

- `smoothstep(edge0, edge1, x)` with `edge0 >= edge1` is **undefined per the GLSL ES spec** —
  driver-divergent, not "just reversed". Same family: `clamp` with `minVal > maxVal`.
- **Reserved words** (`patch`, `sample`, `filter`, `input`, `output`, …) compile on some drivers
  and fail only at runtime link/`VALIDATE_STATUS` on others — check new identifiers against the
  reserved list; "it compiled here" proves nothing.
- Dynamic array/sampler indexing limits (ES 3.0 constraints on non-constant indices) — verify
  against the spec, not the local driver.
- Precision defaults differ between stages and drivers — explicit `precision` on anything
  numerically sensitive.
- Sampling a render target outside [0,1]: **wrap vs clamp is texture state**, not a guarantee —
  check the RT's wrap mode before trusting edge behavior.

### React / R3F

- **DOM ErrorBoundaries cannot catch across the R3F reconciler root** — a throw inside the Canvas
  tree (render, `useFrame`, a setState updater) does not reach a boundary outside the Canvas.
  Any escape hatch (throw-via-setState) must be proven by a forced-error experiment.
- Effect throws escape boundaries entirely (they surface on the window error handler).
- A setState **updater function** that throws, throws at render/commit time, not at call time —
  trace where that actually surfaces.

### CSS / DOM

- `[hidden] { display: none !important }` (UA/reset sheets) **beats inline `style.display`** —
  check visibility toggles against the whole cascade, not the inline style.
- **`transform-origin` vs assumed rotation center** — geometry derived "about the center" is wrong
  the moment the origin is shifted (e.g. a card whose origin is the offset card center); read the
  effective origin, never assume frame-center.

### JS numerics

- **`NaN` fails every comparison** — `if (x <= 0) reject()` passes `NaN` straight through, and so
  does every inverted guard. Numeric-input guards need `Number.isFinite`, not one comparison.
- `Infinity` / `-0` survive naive range guards; `parseFloat` returns `NaN` silently.

### Lifecycle / globals (any JS)

- A **global mutation** (`gsap.ticker.lagSmoothing(0)`, window listeners, body classes, scroll
  state) needs a provably-reached teardown — **including on the exception path** (init throws →
  the fallback must not inherit leaked listeners/state). Force the throw and check.

## Miss ledger (read every run — what "approved" cost us)

Every row is a defect an internal critic APPROVED that an external reviewer (claude gate /
CodeRabbit / the user / `gate-auditor`) then caught. **Rule: when that happens again,
`project-steward` appends a row here AND, if it is a new class, adds it to the sweep above.**

| PR   | What was missed                                                                                                                                             | Rule that would have caught it                                            |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| #714 | 7 reversed-edge `smoothstep(hi, lo, x)` calls — undefined per the GLSL ES spec, driver-divergent — approved as "GLSL clean"                                 | Spec-UB sweep (GLSL descending edges)                                     |
| #714 | Cover-hinge rotation sign drove the board through the page stack — approved off a single top-down screenshot                                                | Empirical verification (non-default camera angle)                         |
| #714 | Bake-failure escape (throw-via-setState inside the R3F Canvas) could never reach the DOM ErrorBoundary — approved by reasoning, disproven by a forced error | Empirical verification (force the error)                                  |
| #713 | Tear/throw phasing — the free rip piece rigid-translated while the seam was still attached                                                                  | Empirical verification (drive the timeline)                               |
| #713 | Global `gsap.ticker.lagSmoothing(0)` leaked past teardown                                                                                                   | Lifecycle/globals sweep (teardown proof)                                  |
| #713 | Scroll-init not exception-safe — leaked listeners into the fallback                                                                                         | Lifecycle/globals sweep (exception path)                                  |
| #714 | GLSL reserved word `patch` broke a shader only at runtime VALIDATE_STATUS — caught live by `gate-auditor`, not the code critic                              | Spec-UB sweep (reserved words fail only at runtime)                       |
| #715 | Leaked `</content>`/`</invoke>` agent artifact at the end of a README                                                                                       | Coverage duty (critic runs on docs-only PRs; read the whole changed file) |
| #715 | `NaN`/`Infinity` passed a `<= 0` guard                                                                                                                      | Spec-UB sweep (JS numerics)                                               |
| #715 | Arrow geometry assumed rotation about frame-center; the CSS `transform-origin` was the shifted card center                                                  | Spec-UB sweep (transform-origin) + empirical verification                 |

## Coverage duty

The sibling critic runs before **EVERY push** on its domain — including docs-only and asset-only
PRs (see #715's README artifact). "Too small to review" is not a category.
