---
name: webgl-standards
description: apps/landing WebGL landing conventions + verified version pins the GL authors and critics load first - R3F v9 / three ~0.185 / postprocessing 6.39.2, the 8-beat t-space camera journey, ink-stroke line-boil, the two-pass post pipeline, capability gate, quality tiers, and the ASCII-only source rule. Load for any change under apps/landing/src/**.
---

# apps/landing WebGL standards

The single source both GL authors (`webgl-author`, `shader-engineer`) and their critics
(`webgl-reviewer`, `gate-auditor`, `webgl-perf-profiler`) read before touching `apps/landing`.
Architecture rationale: `docs/adr/0014-landing-gl-takeover.md`.

## The stack (Next 16 static export)

Full-canvas R3F v9 + three ~0.185 + postprocessing 6.39.2 + Lenis + zustand -- a "living
sketchbook". The camera rides a Catmull-Rom journey; ALL text renders in GL (no DOM text over
the canvas). A prerendered semantic HTML layer stays the SEO/a11y/scroll-height authority.

## Version pins (verified - do NOT drift)

- **postprocessing `^6.39.2` -- NEVER `6.39.0`.** 6.39.0's three peer range excludes 0.185; only
  6.39.2+ accepts the pinned three. A lockfile that resolves 6.39.0 is a blocker.
- **troika / drei Text is TTF-only** -- no woff2. Every drei `Text` needs an explicit `characters`
  prop (the glyph set it will render) or troika silently drops glyphs. Per-word `Text` splits are
  capped to headlines only (each split is its own draw call + SDF atlas).
- **Ink strokes = `Line2` fat lines** from `three/addons/lines` (LineGeometry + LineMaterial),
  with `dashed` + animated `dashOffset` for draw-on. Never plain `THREE.Line`.
- **Line-boil = a VERTEX-SHADER effect** -- per-vertex noise keyed to a STEPPED `uTime` uniform
  (quantize to ~10 fps so the wobble reads hand-drawn). NEVER re-`setPositions()` on the CPU per
  frame (allocation + upload storm).

## The 8-beat t-space journey

Everything scroll-driven is a pure function of a single global `t` in [0,1] (scrub-safe both
directions -- no time-accumulated state driving scroll visuals). The beats, in order:

1. hero 2. slump 3. descent 4. deep-fried 5. godmode 6. features 7. testimonials 8. finale

The DEEP FRIED beat (4) is a deliberate glitch window: Pass B (below) only runs inside it.

## Post pipeline (two passes)

- **Pass A - always on.** One merged custom `SketchbookEffect`: paper grain / fiber, ink bleed,
  warm vignette.
- **Pass B - fried window only.** Bloom (mipmapBlur) + ChromaticAberration + a custom
  `FriedEffect` (posterize + Bayer dither + saturation) + Scanline. Enabled only across the
  deep-fried `t` sub-range; disabled elsewhere via `pass.enabled` / uniform ramps, never by
  rebuilding the composer.

## Capability gate + the semantic layer

- **Gate to GL:** WebGL2 AND fine pointer AND width > 900 AND NOT reduced-motion. Fail any -> the
  legacy prerendered DOM page runs and GL never mounts. Reduced-motion users therefore NEVER reach
  the fried glitch/CA/dither -- that is the comfort contract for this project.
- **Semantic layer is the scroll-height authority.** When GL mounts it gets `visibility:hidden` +
  `inert` -- NEVER `display:none` (that would collapse scroll height and break the journey).

## Quality tiers

| Tier | dpr  | line-boil | stroke budget |
| ---- | ---- | --------- | ------------- |
| HIGH | 2    | 10 fps    | full          |
| LOW  | 1.25 | 8 fps     | halved        |

Degradation ladder (owned by `webgl-perf-profiler`, applied in order, stop at first pass):
halve stroke budget -> boil 10->8 fps -> dpr 2->1.25 -> disable Pass B extras (scanline, dither)
-> grain off. drei `PerformanceMonitor` `onDecline` is the sanctioned adaptive hook if the static
rungs are not enough.

## ASCII-only source (hard rule)

Source files under `apps/landing/src/**` must be pure ASCII. Turbopack's merged source maps crash
on multi-byte characters (the rope bug). Any user-facing copy with non-ASCII glyphs lives
`\uXXXX`-escaped in `src/content/`, never inline in a component.

## Composer pass-toggling gotcha (learned P4, 2026-07-18)

postprocessing's autoRenderToScreen statically pins renderToScreen=true on the ARRAY-LAST pass
at construction. If that pass is ever disabled (recipe windows), the last ENABLED pass renders
offscreen and the canvas goes black outside the window. When any pass toggles enabled: set
autoRenderToScreen=false on the composer and drive renderToScreen per frame in the recipe
(flip only at shoulders where the ramp is zero - no pop). Also verified in 6.39.2 source:
ChromaticAberrationEffect is the CONVOLUTION-class effect (not Bloom - its glow is precomputed
into its own map); mainUv effects (e.g. FriedEffect barrel) cannot share a pass with CA.
