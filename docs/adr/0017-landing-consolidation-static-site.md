---
status: accepted
supersedes: 0016
supersedes-parts-of: 0014
---

# Landing returns to self-contained static site

## Context

Recorded from owner decision 2026-07-20.

TERMINAL VELOCITY ([ADR 0016](0016-terminal-velocity-scroll-film-landing.md)) - a realistic
CG scroll-film retelling the job-hunt story - was built across three milestones and merged to
`main`:

- **M1** (#720): scroll rig, playhead, semantic-layer parity
- **M2** (#721): canyon towers, paper storm
- **M3** (#722): water surface, splash VAT, deep/blackout

M4 (robot, ascent, dawn) was complete-but-uncommitted and preserved in a git stash on branch
`feat/tv-m4-robot`. Production flip (pages.yml deploying the Next.js static export) was held
for owner approval and never executed; the static site remained the deployed target throughout.

On 2026-07-20, the owner **abandoned the film mid-M4** — the visual approval gates were never
passed, and resources redirected. No production impact: the app deployed the static site
throughout the experiment, so the pivot costs only one stash + one consolidation merge.

## Decision

**Landing returns to a self-contained static site.** The Next.js app shell (`apps/landing/
package.json`, `next.config.ts`, build artifacts) is **deleted wholesale**. The 8 legacy static
pages + root `index.html`, CNAME, .nojekyll, and benchmarks moved from `landing/` to `apps/
landing/` form a **standalone static directory** — no build step, not a workspace package, no
Vite/Next wiring.

Everything **structural** from [ADR 0014](0014-landing-gl-takeover.md) that constrained 0016
(Semantic layer, Experience gate, privacy contract) is **retired with the film**. The static
site carries **only** the original `landing/index.html` (CSS, JS, SVG, brand doodles, original
footer links) — all WebGL infrastructure, TERMINAL VELOCITY concepts (playhead, scroll-film,
scenes, quality governor, VAT, shader standards) are abandoned.

**Deploy:** `pages.yml` publishes `apps/landing/` directly (HTTP server default behavior for
`index.html` + sibling pages). No build, no export, no Next machinery.

**Diegetic copy parity** (from ADR 0016): the film retold the static site's story diegetically.
That contract is **void**. The static site keeps all its original links and footer copy
unchanged; no cross-reference to film concepts remains.

## Consequences

- **Removed:** all TERMINAL VELOCITY operational constants (spline knots, quality-governor
  ladders, shader uniforms, foley params) from `.claude/skills/webgl-standards/SKILL.md` —
  that skill document remains but its TERMINAL VELOCITY sections are retired. RIPBOOK sections
  (if any remain) are also retired.
- **Removed:** the experience-gate detection and gl-client runtime in `apps/landing/` (if any
  remains from M1–M3).
- **Removed:** the full M1–M3 commit history from `main` is preserved (they merged to `main`),
  but the commits are now orphaned from any production use. M4 stash on `feat/tv-m4-robot` is
  a recovery point should the film ever be revisited; for the current cycle it is **not
  carried forward**.
- **Removed:** `docs/design/landing-gl-art-direction-brief.md` — described the pre-TERMINAL
  VELOCITY P3 "Living Sketchbook" rebuild and is now moot. Any durable design rationale is
  captured in this ADR's context.
- **Kept:** `docs/CONTEXT.md` entries for TERMINAL VELOCITY, RIPBOOK, and RIPBOOK Exit terms
  are marked superseded (historical glossary for reading the old ADRs), not deleted.
- **Kept:** links to ADR-0014 and ADR-0015 in docs; they remain period documents.

## References

- Supersedes: `docs/adr/0016-terminal-velocity-scroll-film-landing.md` (the film; fully
  retired).
- Supersedes-parts-of: `docs/adr/0014-landing-gl-takeover.md` (the Experience gate and
  Semantic layer machinery that constrained the film are retired; ADR-0014 itself is a period
  document for the 2026-07-18 ideation that birthed 0015/0016).
- Deployed: `apps/landing/` directory; publish via `.github/workflows/pages.yml`.
