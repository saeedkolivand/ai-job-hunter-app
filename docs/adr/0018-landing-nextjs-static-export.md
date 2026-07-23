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

1. **PR1** (shipped): Next.js workspace package, 5 authored pages, static export, public/
   passthrough (benchmarks/dashboards/storybook), version.json release seam, check:parity gate.
2. **PR2** (shipped): Docs-tier DocShell + `/mission-control` full-repo dashboard redesign with
   PAT sign-in + safe-tier write actions.
   2b. **PR2b** (shipped): Architecture-map port from passthrough artifact to typed-data route
   (`src/data/architecture-map.ts`; same URL `/architecture-map`; drift-guard repointed).
3. **PR3** (shipped): OG template relocated from `apps/landing/social-card.html` to
   `scripts/assets/social-card.html` (beside its generator; no longer part of the app).
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

## Addendum: PR2 Implementation

**Next.js version and TypeScript constraint** (shipped 2026-07-20, PR2):
The landing package uses **Next.js 16.2.10** (see `apps/landing/package.json`). The root workspace TypeScript is held
at 6.x for ESLint + typescript-eslint 8.x compatibility (see root `package.json`), landing is pinned to 6.0.3 (see `apps/landing/package.json`),
and other app builds in desktop/extension use 7.x via pnpm per-importer resolution (see respective manifests).
Reason: Next 16's `verifyTypeScriptSetup`
does not recognize TS 7's native-compiler package layout and crashes the build (documented in `apps/landing/package.json#//typescript`). See `pnpm-workspace.yaml` and per-importer `package.json` files for the split configuration. Pinning Next 16 is the stability
anchor; any future upgrade should be gated by `pnpm check:parity` and tested against the
TS 6/7 split constraint.

**Mission control clean rename** (shipped 2026-07-20, PR2):
The `/mission-control` full-repo dashboard replaced the passthrough `ci-dashboard.html`
artifact from `public/`. The old URL path is a **clean rename with no redirect stub** — a
direct owner decision. Future requests to `aijobhunter.app/ci-dashboard/` return 404.
The missio-control feature is now a Next.js typed-data route (`src/data/agent-fleet.ts`
for the agent-system route, and `/mission-control` from `src/app/`). Both are built by
Next, not passthrough artifacts.

**Architecture-map port shipped** (2026-07-21, PR2b):
The architecture-map SVG dashboard was ported from a passthrough artifact (`public/architecture-map.html`)
to a typed-data route (`src/data/architecture-map.ts`). The URL (`/architecture-map`) and flat-file
export (`.html` via static export) are unchanged. The data file replaces the hand-edited passthrough
and is the source of truth for the drift-guard script (`check-landing-drift.mjs`). Design intent and
content unchanged; the port is structural only.

**SECURITY: Origin Invariant for Browser-Stored Tokens** (architectural constraint):
GitHub Pages serves the landing site at `aijobhunter.app` with a meta CSP (`<meta
http-equiv="Content-Security-Policy" ...>`). **Meta CSP has a known limitation: the
`frame-ancestors`, `report-uri`, and `sandbox` directives are inert in meta tags** (they
only work in HTTP headers). This means framing attacks _cannot_ be mitigated by meta CSP.
**Mitigation:** A frame-buster script (inline `<script>` in the page's HTML) tests
`window.self !== window.top` and navigates to `self` if framed — this stops clickjacking
on Pages even when frame-ancestors is ignored.

**Hard constraint:** No third-party JavaScript must ever be allowed on the `aijobhunter.app`
origin. Any foreign script (a polyfill from a CDN, a third-party widget, an analytics
snippet) can read `localStorage`, and the mission-control dashboard stores a GitHub PAT
in localStorage for signed-in users. Because meta CSP cannot enforce script-src, **the
only protection is the absence of third-party script tags in the HTML**. All scripts are
either:

- Inline `<script>` (baked into the Next.js build output), or
- Same-origin JS modules fetched from `/\_next/` (Next.js runtime).

**The origin invariant extends to stylesheets:** all fonts and third-party styles are self-hosted from `public/fonts/` to prevent external stylesheet links.

Verify on every landing page update that no external `<script src="https://…">` or
`<link href="https://…" rel="stylesheet">` tags are present in the built HTML (the
parity gate catches structural changes). This origin invariant is non-negotiable for
browser-stored secrets.

## Addendum: PR4 Metrics-snapshot data plane

**Nightly metrics snapshot and GitHub API live fallback** (shipped 2026-07-21, PR4):
The mission-control dashboard now fetches data from a nightly snapshot (`apps/landing/public/metrics/`) with live GitHub API fallback. A new `.github/workflows/metrics-snapshot.yml` runs nightly (03:17 UTC) and on manual dispatch: it bakes 7 GitHub REST API payloads into JSON files and commits them to main via SSH over the `RELEASE_DEPLOY_KEY` (the default `GITHUB_TOKEN` is not a ruleset bypass actor and its push to protected main is rejected). The `commits.json` snapshot is uniquely field-allowlisted (via jq) to strip git author/committer names and emails before public upload — the client consumes only sha, message, date, and login. Missing or stale snapshots degrade gracefully via per-key live fallback (dashboard shows an honest freshness line from `meta.json`). The push is marked `[skip ci]` + the pages.yml workflow is dispatched explicitly to keep the deploy intentional and avoid redundant CI runs. This approach trades real-time accuracy for load relief and reproducible dashboard renders across multiple deployments. The snapshot mode is transparent to users via a single `liveOrSnapshot` seam in mission-control's data source configuration (`dataSource: { mode: 'snapshot', snapshotBase: '/metrics' }`); the API data-fetch path remains available for client sign-in scenarios (users with fine-grained PATs can opt into live mode for latest data).

## Addendum: PR1–PR3 Raw-HTML Pipeline Retirement

**Conversion of all 5 marketing pages to TSX and retirement of the fragment pipeline** (completed 2026-07-23, PRs #872, #879, this PR):

The static-HTML fragment pipeline (`src/content/`, `src/components/RawHtml.tsx`, `src/lib/content.ts`, and per-page `body.html` files) has been fully retired across three sequential PRs. All five authored marketing pages (home, creature, how-it-works, privacy, download) are now TSX components (`src/components/*/Body.tsx` or `src/app/*/page.tsx`). **Rendered DOM is byte-identical** to the legacy static site — verified per route via `scripts/diff-dom.mjs` against pre-refactor baselines.

**Structural changes:**

- **Components**: `HomeBody`, `CreatureBody`, `HowItWorksBody`, `PrivacyBody`, `DownloadBody` are server components with no props, each building its markup from constant JSX literals (one-to-one conversion of the original `body.html` fragments, preserving class names, attributes, SMIL, and Unicode verbatim).
- **Shared UI**: `SiteFooter` and `BackLink` components extracted and reused across pages (rendered byte-identical to the original).
- **CSS**: Moved from `src/content/*/*.css` to `src/styles/*.css` (e.g., `src/content/how-it-works/styles.css` → `src/styles/how-it-works.css`). One comment updated to reference the `display:contents` root wrapper. Content-identical otherwise (whitespace normalized by Prettier per repo convention).
- **Fonts**: Hoisted to `src/app/layout.tsx` (loaded once, not per-page).
- **Shared tokens**: `marketing-tokens.css` and `marketing-base.css` consolidated and applied via `PageStyle` at page level.
- **Link list**: The parity gate's `EXPECTED_HREFS` literal in `scripts/check-parity.mjs` is now the canonical source for marketing links (was derived from parsed `body.html` fragments). Editing a marketing link now requires conscious update of this frozen list.

**Parity gate**: `pnpm check:parity` remains the permanent non-optional guard — it ensures Next export output byte-matches the legacy static-site layout per page via `scripts/diff-dom.mjs` element-by-element comparison (inclusive of entity-normalized inline styles). Blind spot: `diff-dom.mjs` drops whitespace-only text nodes, so lost inline word-gaps between adjacent inline elements are invisible to the diff. Compensating checks: explicit `{' '}` discipline in JSX, whole-page whitespace-collapsed textContent comparison against the baseline during review, and per-page structural render tests. A regression test (`scripts/diff-dom.test.mjs`) documents the colon-spacing fix for multi-declaration inline styles.

**Deleted files**: `src/content/` (directory), `src/components/RawHtml.tsx`, `src/lib/content.ts`. Repo-wide grep confirms no remaining references to `RawHtml` or `readContent` outside historical commit messages and the already-converted pages' provenance comments.

## References

- Supersedes-parts-of: `docs/adr/0017-landing-consolidation-static-site.md` (the no-build-step
  property; the directory consolidation is reaffirmed).
- Related: `.github/workflows/pages.yml` (deploy step), `apps/landing/next.config.ts`
  (static-export config), `apps/landing/scripts/check-parity.mjs` (parity gate), `apps/landing/src/data/agent-fleet.ts` (typed-data routes).
