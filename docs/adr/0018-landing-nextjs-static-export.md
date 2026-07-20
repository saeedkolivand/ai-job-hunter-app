---
status: accepted
supersedes-parts-of: 0017
---

# Landing migrates to Next.js static export with real routes

## Context

Recorded from owner decision 2026-07-20.

[ADR 0017](0017-landing-consolidation-static-site.md) consolidated landing to a **self-contained
static site** (no build step, pure HTML/CSS/JS passthrough). That decision was made and executed
the same week, after TERMINAL VELOCITY was abandoned mid-M4. However, the static-site-only
approach proved inflexible for planned features: future state management, dynamic route data,
and client-side components (on-demand page chunks, interactivity beyond foley JS, runtime
config).

On 2026-07-20, the owner reversed the no-build-step constraint within the static-site
consolidation. The **directory consolidation** (landing lives in `apps/landing/` as a workspace
package, separate from the desktop app) remains sound. The **no-build-step property is
superseded** — the landing now has a build step and is managed as a Next.js 15 package.

## Decision

**Landing is a Next.js 15 static-export app** (`output: 'export'`), deployed to GitHub Pages as
flat HTML/CSS/JS files (byte-shape parity with the legacy hand-authored static site via
`trailingSlash: false`). It is **not a server app** — no server features, no runtime, no SSR.
All 5 authored pages are routes (`src/app/page.tsx`, `creature/`, `how-it-works/`, `privacy/`,
`download/`). Third-party artifacts (CI-generated dashboards, benchmarks, storybook) remain in
`public/` as passthrough — copied verbatim at build time, not built by Next.

**Parity gate:** `pnpm check:parity` (run pre-push + in CI) verifies that the Next export
output byte-matches the legacy static site layout per page. This is a **permanent, non-optional
guard** — it ensures no URL changes, no surprise structural diffs, and reversibility if needed.

**Release seam:** `src/data/version.json` is a build-time artifact, populated on release. The
homepage and `/download` page read it for client-side freshness checks (e.g. "new version
available"). No API call; the JSON is baked at build time and deployed with the static export.

**Tiers:** Two visual skins are locked:

- **Marketing tier** (pages 1–4: home, creature, how-it-works, privacy) — untouched from the
  hand-authored design; brand, tone, footer links all preserved.
- **Docs tier** (planned PR2–PR4) — `/mission-control` full-repo dashboard + `docs/` content
  pages use a unified look (unifying the disparate design-system, landing, and agent-system
  pages). The two tiers share no visual language; marketing skin is protected.

**Delivery chain (four PRs):**

1. **PR1** (current): Next.js workspace package, 5 authored pages, static export, public/
   passthrough (benchmarks/dashboards/storybook), version.json release seam, check:parity gate.
2. **PR2** (pending): Docs-tier DocShell + `/mission-control` full-repo dashboard redesign with
   PAT sign-in + safe-tier write actions.
3. **PR3** (pending): OG template relocation (marketing tier's og-card.jpg or docs tier's dynamic
   og-image).
4. **PR4** (pending): Nightly metrics-snapshot data plane (autonomous update of data.js + CI step).

## Consequences

- **Removed from ADR-0017's scope:** The "no build step" constraint on landing is lifted. All
  other properties (directory at `apps/landing/`, workspace package, deploy via `pages.yml`,
  marketing tier design fixed, privacy contract) are reaffirmed.
- **Kept:** The full history of the static-site-only migration (ADR-0017, commits) is preserved
  as a checkpoint — should the decision reverse again, it is a known revert target.
- **Build tooling:** `pnpm build` in `apps/landing/` runs `next build`, producing `out/` with
  flat files (no `index.html` within directories — just `.html` files at the root and per-route).
  GitHub Pages serves them via the default index-to-directory fallback.
- **Dependency upgrade path:** Next.js 15.x is the pinned version. Future upgrades should be
  gated by the parity check — any Next version bump that changes output layout breaks the gate
  and must be tuned (trailingSlash, outputFileTracingRoot, etc.) until parity is restored.
- **No server features:** `output: 'export'` + ESLint ban blocks any attempt to add server-side
  code (API routes, middleware). Future interactive pages must use React client components only.
  If true server features become necessary, a separate backend (or a Tauri localhost bridge) is
  the only option — this app never becomes a full-stack Next deployment.

## References

- Supersedes-parts-of: `docs/adr/0017-landing-consolidation-static-site.md` (the no-build-step
  property; the directory consolidation is reaffirmed).
- Related: `.github/workflows/pages.yml` (deploy step), `apps/landing/next.config.ts`
  (static-export config), `apps/landing/scripts/check-parity.mjs` (parity gate).
