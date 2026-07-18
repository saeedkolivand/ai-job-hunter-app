---
status: accepted
amended-by: 0015
---

# Landing becomes a built GL experience with a semantic fallback

> **Amended by [ADR 0015](0015-ripbook-notebook-landing.md) (2026-07-18).** 0015 replaces
> this ADR's landing _experience_ - the 8-beat scroll-scrubbed camera Journey, the "beat copy
> in a screen-space DOM overlay" ruling, and the deep-fried Pass B glitch set-piece - with the
> RIPBOOK notebook (9 rippable pages, all copy as in-canvas SDF text, one always-on post chain).
> Everything _structural_ here still binds: the Next 16 static export, the Semantic layer as the
> SEO/a11y/scroll-height authority, the single Experience gate, the `landing/` passthrough merge,
> and the staged flip procedure (delete `landing/index.html` only at the owner-approved flip).

## Context

The landing page shipped as a single hand-authored `landing/index.html` served
verbatim by GitHub Pages (no build step). The next landing iteration is a
full-canvas WebGL experience - a scroll-scrubbed camera Journey through 8 Beats -
which cannot live in one hand-maintained HTML file: it needs a component tree, a
module graph, and a bundler.

Recorded from the grill session on 2026-07-17. The constraint that shaped every
option: the content must stay fully readable and crawlable when WebGL does not run
(no WebGL2, coarse pointer, narrow viewport, or reduced-motion), and the deploy
must stay a static artifact on GitHub Pages.

## Decision

The landing index becomes a **Next 16 static-export app** at `apps/landing`
(package `@ajh/landing`). It prerenders a **Semantic layer** - the content HTML
that is always in the DOM and owns SEO, accessibility, and scroll height - and
mounts the full-canvas WebGL experience on top of it only when the single
**Experience gate** passes. The exported site is `apps/landing/out`; the
`landing/` directory becomes **Passthrough files** copied verbatim into the export
by the postbuild merge-passthrough script.

## Considered options

1. **GL background sandwich behind the DOM.** A canvas fixed behind the existing
   DOM content. Rejected: reduces the experience to wallpaper - it cannot drive a
   scroll-scrubbed camera Journey that the content sits inside.
2. **importmap vanilla three, no build.** Keep the no-build deploy, load three via
   an importmap. Rejected: no bundling/tree-shaking/typecheck, and the growing
   module graph is unmaintainable as one hand-authored file.
3. **Script-only overlay bundle, index.html still hand-authored.** Bundle just the
   GL overlay, keep authoring `index.html` by hand. Rejected: splits ownership of
   the DOM between a hand file and a bundle and leaves the semantic content
   un-built and drift-prone.

## Consequences

- Pages now has a build step: see `.github/workflows/pages.yml` (build
  `@ajh/landing`, upload `apps/landing/out`).
- The passthrough merge means `landing/` inputs still ship verbatim; at the flip
  (P7) `landing/index.html` is deleted and the app owns the index.
- Delivery is staged behind per-phase gates rather than one big cutover.
- The Semantic layer and Experience gate keep the no-WebGL visitor whole.

## References

- Glossary: `docs/CONTEXT.md` (Semantic layer, Experience gate, Journey, Beat,
  Passthrough files, Line boil).
- App: `apps/landing`; passthrough source: `landing/`; deploy: `.github/workflows/pages.yml`.
