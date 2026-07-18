---
name: webgl-standards
description: apps/landing WebGL landing conventions + verified version pins the GL authors and critics load first - R3F v9 / three ~0.185 / postprocessing 6.39.2 / gsap+lenis, the RIPBOOK 9-page p-space scroll model, the uBoil stepped-boil contract, the rip system, the single always-on post chain, capability gate, quality tiers, and the ASCII-only source rule. Load for any change under apps/landing/src/**.
---

# apps/landing WebGL standards (RIPBOOK)

> **⚠ SUPERSEDED CONCEPT - awaits its TERMINAL VELOCITY revision.** RIPBOOK was abandoned
> mid-M3 (2026-07-18); the landing is now **TERMINAL VELOCITY**, a realistic CG scroll-film
> (`docs/adr/0016-terminal-velocity-scroll-film-landing.md`). Every RIPBOOK-specific rule below
>
> - the 9-page p-space model, the Rip system, `uBoil`/line-boil, in-canvas troika SDF text, the
>   single ink post chain, and the RIPBOOK budgets - is **superseded by ADR 0016** and no longer
>   the contract. What still holds: the Next 16 static-export stack, the capability gate + semantic
>   layer, and the ASCII-only source rule. This skill will be rewritten for TERMINAL VELOCITY
>   (WebGL2 lane, VAT playback, Gerstner water, godrays, glTF-clip scrub, quality governor) by the
>   GL authors in a later task; until then treat the numbers here as historical.

The single source both GL authors (`webgl-author`, `shader-engineer`) and their critics
(`webgl-reviewer`, `gate-auditor`, `webgl-perf-profiler`) read before touching `apps/landing`.
Architecture rationale: `docs/adr/0014-landing-gl-takeover.md` as amended by
`docs/adr/0015-ripbook-notebook-landing.md` (RIPBOOK).

## The stack (Next 16 static export)

Full-canvas R3F v9 + three ~0.185 + postprocessing 6.39.2 + gsap + lenis + zustand -- **RIPBOOK**,
a kraft-paper **notebook on a dark desk**, camera looking down ~25 degrees. Every story beat is a
rippable **Page**; scroll plays a Page then Rips it out; ripped pages persist as a Desk pile
(the progress indicator). **ALL copy renders in GL as SDF text** (troika) -- no DOM text over the
canvas. A prerendered semantic HTML layer stays the SEO/a11y/scroll-height authority.

## Version pins (verified - do NOT drift)

| Package             | Pin                    | Note                                                                               |
| ------------------- | ---------------------- | ---------------------------------------------------------------------------------- |
| three               | 0.185.1                | the `~0.185` line R3F v9 + drei target                                             |
| @react-three/fiber  | 9.6.1                  | pairs with React 19                                                                |
| @react-three/drei   | 10.7.7                 |                                                                                    |
| postprocessing      | 6.39.2 -- NEVER 6.39.0 | 6.39.0's three peer range excludes 0.185; a lockfile resolving 6.39.0 is a blocker |
| troika (three-text) | 0.52.4                 | TTF-only; explicit `characters`                                                    |
| gsap                | 3.15.0                 | ONE ScrollTrigger master timeline, `scrub:true`                                    |
| lenis               | 1.3.25                 | owns document scroll; synced to ScrollTrigger                                      |
| framer-motion       | (DOM-only)             | loader / mute toggle / a11y overlay ONLY -- never over the canvas                  |
| typescript          | 6.0.3                  |                                                                                    |

Caveats that bite:

- **troika / drei `Text` is TTF-only** -- no woff2. Every `Text` needs an explicit `characters`
  prop or troika silently drops glyphs. Per-word `Text` splits are **headlines only** (each split
  is its own draw call + SDF atlas).
- **Ink strokes = `Line2` fat lines** from `three/addons/lines` (LineGeometry + LineMaterial),
  `dashed` + animated `dashOffset` for draw-on. Never plain `THREE.Line`. Salvaged `doodles.json`
  SVG paths feed these for 2D page-surface ink.
- **Line boil is a VERTEX-SHADER effect** keyed to the stepped `uBoil` uniform (below) -- NEVER
  re-`setPositions()` on the CPU per frame (allocation + upload storm).

## The 9-page p-space model

Everything scroll-driven is a pure function of a single global `t` in `[0,1]` (scrub-safe both
directions -- no time-accumulated state driving scroll visuals). Page `i` is **active** when
`t` is in `[i/9, (i+1)/9]`. The 9 pages, in order, with their Rip exit:

| #   | Page         | Exit                                                                     |
| --- | ------------ | ------------------------------------------------------------------------ |
| 0   | Cover        | hinge-opens                                                              |
| 1   | Slump        | corner tear                                                              |
| 2   | DescentA     | horizontal mid-rip                                                       |
| 3   | DescentB     | vertical tear                                                            |
| 4   | Fried        | crumple + toss (ink-native rage, NO glitch pass)                         |
| 5   | AreYouSure   | folds into a paper plane                                                 |
| 6   | Features     | diagonal tear                                                            |
| 7   | Testimonials | perforation zip                                                          |
| 8   | Godmode      | -> back cover; pencil signature + stamp, **no rip**, two-moment timeline |

**Page-local `p`.** Map `t` within the active page to `p` in `[0,1]`: `p` in `[0,0.72]` **plays**
the page, `p` in `[0.72,1]` scrubs the page's **exit**. A **Rip** is the usual exit (pages 1-7);
page 0's exit is the cover **hinge-open** and page 8's exit is the **pencil signature + stamp +
back-cover close** (no rip) -- same `[0.72,1]` window, different exit. Both regions are pure `f(t)`
and fully reversible.

**One timeline, channels bridge.** Lenis owns document scroll; a **single** GSAP ScrollTrigger
`scrub:true` master timeline (absolute tweens ONLY -- no `+=`) writes into **preallocated
channels** + the zustand store. GL reads `store.getState()` per frame; React never re-renders per
frame. Lenis->ScrollTrigger sync: drive `lenis.raf` from `gsap.ticker`, call `ScrollTrigger.update`
on `lenis.on('scroll', ...)`, and let the semantic layer's 9 x 140vh sections own scroll height.

## Post pipeline (RIPBOOK chain - single, always-on)

Built on the **raw** postprocessing composer. Order:

1. `RenderPass`
2. `EffectPass` -- **tilt-shift DOF** (focus band on the active page)
3. `EffectPass` -- **merged Crease + PaperGrain + Vignette** (Sobel crease lines; paper grain
   `<= 0.035`, stepped at boil fps; warm vignette)

with `multisampling: 4` (MSAA; SMAA fallback where MSAA is unavailable). **No pass EVER toggles at
runtime.** **No bloom, no chromatic aberration, no halftone** -- the deep-fried Pass B set-piece is
retired (ADR 0015); the Fried page gets ink-native aggression instead (editor-red rage strokes,
heavy boil amplitude, dense cross-hatch), not effect passes.

## uBoil singleton-uniform contract

`uBoil` is **one uniform object shared by reference** across every ink/hatch/crease material. The
**single writer** is the composer's `priority 1` `useFrame`; it sets
`uBoil = floor(time * boilHz) / boilHz` once per frame, where `boilHz` is the **active quality
tier's** boil rate (HIGH 10, LOW 8 -- see Quality tiers). It is **not** a fixed 10 Hz. No material
owns its own boil clock, and nothing else writes it. Re-seed all ink jitter from `uBoil` so the
whole page boils on the same step.

## Boil-vs-smooth split

The hand-drawn **jitter** (ink re-seed, cross-hatch, crease wobble, grain) steps on `uBoil` so it
reads hand-drawn. The **camera, physics, rip bend, and DOF** run off real time and stay **smooth
60fps**. Never step the camera or the rip morph on `uBoil` -- stepping motion, not just ink, looks
broken.

## Rip system invariants

- **Pre-split at load.** Each **rippable** page (1-7) is authored as a tear mesh at load time
  (never re-triangulated per frame). Page 0 (cover hinge) and page 8 (signature + stamp + back-cover
  close) use their own exit meshes, not the tear/morph rig.
- **Vertex-shader bend + morph targets.** The Rip is a vertex-shader bend plus morph-target
  crumple/fold driven by `p` in `[0.72,1]` -- pure `f(t)`, so scrubbing back below 0.72 fully
  reassembles the page.
- **Pile via `.count`.** The Desk pile is one `InstancedMesh`; reveal ripped pages by raising
  `.count`, never by adding meshes.
- **Dispose + prefetch.** Dispose a page's geometry when its page-distance > 2, and rebuild on
  approach (prefetch). A disposal/prefetch bug reads as a black/blank page.

## Paper-bake budget

**One shared kraft bake** (albedo + normal + roughness) baked once at load, plus **one seeded
stain/smudge atlas** -- reused across all pages. **Never per-page 4096^2 bakes.** HIGH bakes at
4096^2, LOW at 2048^2. No downloaded textures.

## Budgets (the numbers other docs point at) + visibility scoping

This skill owns the RIPBOOK performance budgets; ADR 0015 and CONTEXT.md point here instead of
copying them:

- **Frame rate:** 60fps @ 1440p on M1 / GTX 1660 through every page and exit (incl. the crumple rip).
- **JS bundle:** <= 1.3 MB gzipped.
- **Draw calls:** < 120 -- probe with `renderer.info.render.calls`.

Visibility scoping keeps the draw-call budget: only the **active page +/-1** is mounted; distant
pages are disposed (see Rip system). The pile is one instanced draw; per-word troika splits are
headlines-only precisely because each is its own draw call.

## Capability gate + the semantic layer + the a11y overlay

- **Gate to GL:** WebGL2 AND fine pointer AND width > 900 AND NOT reduced-motion, **and** a
  `flipped()` pre-launch guard until the production flip. Fail any -> the legacy prerendered DOM
  page runs and GL never mounts. Reduced-motion users therefore never reach the boil/exit motion.
- **Semantic layer is the scroll-height authority.** When GL mounts it gets `visibility:hidden` +
  `inert` -- NEVER `display:none` (that collapses scroll height and breaks the scroll rig). It keeps
  owning SEO + machine-readable copy while hidden; it is NOT the interactive surface once GL runs.
- **a11y overlay is the accessible interface while GL runs.** Because copy is in-canvas SDF (opaque
  to the a11y tree), a **visually-hidden but focusable** DOM overlay carries the accessibility: REAL
  `<a>`/`<button>` elements positioned over the canvas hotspots (CTA, film hints, footer / store /
  sponsor links, the sound / "mute the guy" toggle, doodle pokes, dialog buttons), a **skip-link
  first in tab order**, and an `aria-live` region that mirrors the gag/speech-bubble text as it
  changes. The overlay is keyboard + screen-reader operable even though it is visually hidden; the
  canvas itself stays `aria-hidden`. Do not put the interactive controls only on the canvas.

## Quality tiers

| Tier | dpr  | boil   | paper bake | stroke budget |
| ---- | ---- | ------ | ---------- | ------------- |
| HIGH | 2    | 10 fps | 4096^2     | full          |
| LOW  | 1.25 | 8 fps  | 2048^2     | halved        |

Degradation ladder (owned by `webgl-perf-profiler`, applied in order, stop at first pass): halve
stroke budget -> boil 10->8 fps -> dpr 2->1.25 -> reduce tilt-shift DOF sample quality -> grain
off. drei `PerformanceMonitor` `onDecline` is the sanctioned adaptive hook if the static rungs are
not enough. (There is no "disable Pass B" rung any more -- the post chain never toggles.)

## ASCII-only source (hard rule)

Source files under `apps/landing/src/**` must be pure ASCII. Turbopack's merged source maps crash
on multi-byte characters (the rope bug). Any user-facing copy with non-ASCII glyphs lives
`\uXXXX`-escaped in `src/content/`, never inline in a component.

## Scrub-safety contract

- Everything scroll-driven is a pure function of `t` -- **no `+=` accumulation** anywhere in the
  scroll path (GSAP tweens absolute; store writes idempotent for a given `t`).
- Read scroll state per-frame via `store.getState()` inside the loop -- **never a hook selector**
  for per-frame values (a selector re-renders the tree every frame).
- **One** `priority 1` `useFrame`, owned by the composer, drives the render + `uBoil`; every other
  animation `useFrame` stays at the default priority `0`.
- **InstancedMesh:** `setColorAt()` then `instanceColor.needsUpdate = true`; `setMatrixAt()` then
  `instanceMatrix.needsUpdate = true`. `instanceColor` is `null` until the first `setColorAt`, and
  any instance you never set renders WHITE -- set every instance (the pile reveals via `.count`,
  but every slot must be initialized first).

## autoRenderToScreen historical note

postprocessing's `autoRenderToScreen` statically pins `renderToScreen=true` on the array-last pass
at construction; if that pass is ever disabled the last enabled pass renders offscreen and the
canvas goes black. RIPBOOK's chain **never toggles a pass**, so this hazard does not arise at
runtime -- but keep `composer.autoRenderToScreen = false` and set `renderToScreen` explicitly on the
last pass, as a durable safety default (learned P4, 2026-07-18).
