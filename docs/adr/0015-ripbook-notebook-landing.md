---
status: accepted
supersedes-parts-of: 0014
---

# Landing rebuilds as RIPBOOK - a full-WebGL kraft notebook that rips out pages

## Context

Recorded from the grill session on 2026-07-18 (owner-approved).

The GL landing shipped through P0-P5 as the "living sketchbook" of
[ADR 0014](0014-landing-gl-takeover.md) - a scroll-scrubbed camera Journey through
8 Beats - then was reset to an empty `apps/landing` in #710. The owner judged the
sketchbook journey **not premium enough** and approved a rebuild under a new concept,
**RIPBOOK**. The old Next 16 app at git `66c36c68` remains **salvage material**, not the
baseline.

Everything structural in 0014 still constrains this rebuild: the deploy stays a Next 16
**static export** on GitHub Pages, and the content must stay fully readable and crawlable
when WebGL does not run. RIPBOOK changes the _experience_, not that machinery.

## Decision

The landing becomes **RIPBOOK**: a full-WebGL kraft-paper **notebook** on a dark desk,
camera looking down ~25 degrees. Every story beat is a notebook **Page**; scrolling plays
the Page then **Rips** it out; ripped pages persist as a **Desk pile** that is the progress
indicator (with an odometer).

**The 9 pages (index - name - exit):**

| #   | Page         | Exit style                                                            |
| --- | ------------ | --------------------------------------------------------------------- |
| 0   | Cover        | hinge-opens                                                           |
| 1   | Slump        | corner tear                                                           |
| 2   | DescentA     | horizontal mid-rip                                                    |
| 3   | DescentB     | vertical tear                                                         |
| 4   | Fried        | crumple + toss                                                        |
| 5   | AreYouSure   | folds into a paper plane                                              |
| 6   | Features     | diagonal tear                                                         |
| 7   | Testimonials | perforation zip                                                       |
| 8   | Godmode      | -> back cover (pencil signature + stamp, NO rip; two-moment timeline) |

**Scroll rig.** Lenis owns document scroll; the prerendered semantic layer stays the
scroll-height authority (9 x 140vh sections). Lenis is synced to **one** GSAP ScrollTrigger
`scrub:true` master timeline (absolute tweens only) writing into preallocated channels + a
zustand store; GL reads `store.getState()` per frame. Page-local progress `p` in `[0,0.72]`
plays the Page, `[0.72,1]` scrubs the Rip; the whole thing is a pure function of scroll and
**fully reversible**. Contract in `.claude/skills/webgl-standards/SKILL.md`.

**Full-GL text.** All copy renders as SDF text **in-canvas** via troika (TTF-only, explicit
`characters`). There is no DOM text over the canvas. (Supersession below.)

**Post chain (strict, always-on, nothing toggles).** RenderPass -> tilt-shift DOF (focus
band on the active page) -> paper/grain composite (grain <= 0.035, stepped at boil fps) ->
vignette, with MSAA 4x (SMAA fallback). **No bloom, no chromatic aberration, no halftone.**
Built on the **raw** postprocessing composer (6.39.2 pin unchanged).

**Hand-drawn look.** A global stepped **boil** uniform (`uBoil = floor(time*10)/10`)
re-seeds all ink jitter while camera/physics stay smooth 60fps; 3-band toon + procedural
cross-hatch atlas; inverted-hull screen-constant outlines; Sobel crease lines; pencil grain;
seeded smudge / eraser-ghost decals; procedural kraft paper **baked once at load** (no
downloaded textures).

**Audio.** The legacy gibberish-voice synth is ported verbatim to TS, plus new procedural
paper **Foley** (rip / crumple / whoosh / scribble / stamp). A mute toggle ("mute the guy").

**Characters.** True 3D **procedural** meshes, no model files: a sad protagonist that
degrades page by page, a recruiter creature, a resume robot, four recruiters, an instanced
swarm, and the godmode guy.

**Copy parity.** 1:1 with `landing/index.html`, enforced by a bidirectional diff script (a
gate step from M3).

**Salvage strategy.** The old app at `66c36c68` is reference-only. `src/data/doodles.json`
(extracted legacy SVG stroke art) is salvaged for page-surface 2D ink, lazily loaded; the
cast is rebuilt as procedural 3D (no salvaged meshes).

**New dependencies.** `gsap 3.15.0` + `lenis 1.3.25` (scroll rig); `framer-motion` **DOM-only**
(loader, mute toggle, a11y overlay - never over the canvas). Version pins live in
`.claude/skills/webgl-standards/SKILL.md`.

**Budgets.** 60fps @ 1440p on M1 / GTX 1660 through every rip; JS <= 1.3 MB gz; draw calls
< 120.

**Delivery.** PRs land docs -> scaffold -> M1..M6 and merge autonomously. The **production
flip** PR (pages.yml builds `apps/landing/out` and deletes only `landing/index.html`) is
**opened but held for owner approval** (the #707 pre-launch-gate precedent).

## Supersessions

This ADR supersedes three previously locked rulings; each is named so the reader can find
what changed:

1. **The 8-beat t-space camera Journey** (0014 Context; the "8-beat t-space journey" section
   of `webgl-standards`). Replaced by the **9-Page notebook + Rip** model - global `t` still
   drives everything, but page `i` owns `t` in `[i/9,(i+1)/9]` with the play/rip `p`-space
   split at 0.72.
2. **The art-brief ruling "beat copy in a screen-space DOM overlay."** Replaced by **all copy
   as in-canvas SDF text** (troika). The semantic layer keeps the _machine-readable_ copy;
   the _visible_ copy is GL.
3. **The deep-fried Pass B glitch set-piece** (Bloom + ChromaticAberration + FriedEffect
   posterize/dither + Scanline; the "Post pipeline (two passes)" section of `webgl-standards`).
   **Fully retired.** The Fried page instead uses **ink-native aggression** - editor-red rage
   strokes, heavy boil amplitude, dense cross-hatch, crumple exit - under the single always-on
   post chain. No effect ever toggles at runtime.

## Consequences

- The two-pass composer (Pass A always-on / Pass B fried-window) collapses to **one always-on
  chain**; the `autoRenderToScreen` pass-toggle hazard no longer arises at runtime (kept as a
  historical note in `webgl-standards`).
- **GSAP scrub discipline is load-bearing.** Every tween absolute, no `+=` accumulation, one
  master timeline - a single relative tween breaks reversibility. Enforced by the scrub-safety
  contract + the gate's rip-reversal check.
- **troika per-glyph boil risk.** Stepping every glyph's jitter at boil fps can spike draw
  calls / re-layout; the fallback is per-word (not per-glyph) boil on headlines only, with body
  copy static under boil.
- **Budget enforcement.** Draw calls < 120 and JS <= 1.3 MB gz are gate-enforced (`renderer.info`
  probe + bundle check); the procedural bakes and rip pre-splits must fit inside them.
- **Rip lifecycle risk.** Pages dispose at page-distance > 2 and rebuild on approach; a disposal
  or prefetch bug reads as a black/blank page - covered by the gate's disposal + reversal checks.
- Reduced-motion / no-WebGL2 / coarse-pointer / narrow visitors never reach any of this: the
  Experience gate + semantic layer of 0014 still hold them whole.

## References

- Amended parent: `docs/adr/0014-landing-gl-takeover.md`.
- Glossary: `docs/CONTEXT.md` (Page, Rip, p-space, Desk pile, Foley, Gibberish voice; and the
  still-binding Semantic layer, Experience gate, Passthrough files, Line boil).
- Implementation rules: `.claude/skills/webgl-standards/SKILL.md`; gate procedures:
  `.claude/skills/webgl-gate-audit/SKILL.md`.
- Salvage source: git `66c36c68`; copy source of truth: `landing/index.html`; app: `apps/landing`.
