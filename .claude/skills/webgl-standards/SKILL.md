---
name: webgl-standards
description: apps/landing WebGL conventions + verified version pins the GL authors and critics load first for TERMINAL VELOCITY - the realistic CG scroll-film. Owns the tunable constants ADR 0016 defers here - R3F v9 / three 0.185 / postprocessing 6.39.2 / three-good-godrays / gsap+lenis / detect-gpu / zustand, the one-shot playhead + 9-scene scroll map, the WebGL2 lane, the scroll rig, glTF-clip camera/character scrub, VAT splash playback, Gerstner water + godrays, the instanced paper storm, the filmic post chain, per-tier budgets + quality governor, the capability gate + semantic layer + a11y overlay, and the ASCII-only source rule. Load for any change under apps/landing/src/**.
---

# apps/landing WebGL standards (TERMINAL VELOCITY)

The single source both GL authors (`webgl-author`, `shader-engineer`) and their critics
(`webgl-reviewer`, `gate-auditor`, `webgl-perf-profiler`) read before touching `apps/landing`.
Experience contract + rationale: `docs/adr/0016-terminal-velocity-scroll-film-landing.md` (the
source of truth). Still-binding structural machinery (Next 16 static export, Semantic layer,
Experience gate, `landing/` passthrough, staged flip): `docs/adr/0014-landing-gl-takeover.md`.
**The ADR holds the decisions; this skill holds the tunable numbers and their current starting
values.** Every constant below marked _tunable_ may move within its ADR envelope during M1..M6.

## What TERMINAL VELOCITY is

A realistic CG **scroll-film** (~2:40). Scroll IS the **playhead** (native-scroll -> timeline
0->1), fully reversible: scrolling up rewinds. One camera, one continuous vertical world, zero
cuts - a burned-out job hunter tips off his chair at 2:47 AM and falls through a canyon of
rejection towers into a paper ocean to the lightless bottom, where a robot finds him and carries
him back up to dawn. It does everything except press send (the finale SEND button is the one real
action). Copy is diegetic and in-world; the prerendered **Semantic layer** keeps the
machine-readable and crawlable copy. RIPBOOK (the notebook / rip / boil / in-canvas SDF model) is
fully retired - see ADR 0016 Supersessions; do not carry any RIPBOOK rule forward.

## Ownership split (who edits what)

- **`webgl-author`**: the R3F scene graph, the scroll rig + store + governor wiring, instancing
  and DataTexture bookkeeping, glTF-clip mixer scrub, asset loading + KTX2/DRACO decoder wiring,
  the a11y overlay + semantic layer, camera/animation `useFrame` loops.
- **`shader-engineer`**: all GLSL, custom postprocessing `Effect`/`Pass` classes, `onBeforeCompile`
  material patches, `src/post/**`, the Gerstner/caustics/godrays/VAT-decode shader math. Hand those
  to `shader-engineer`; the author wires the material + uniforms, never authors the shader.

## The stack + version pins (verified 2026-07-18 - do NOT drift)

Full-canvas R3F v9 + three 0.185 on a **WebGL2 lane** for v1 (TSL/WebGPU is a deliberate tier-up
experiment only - it would force rebuilding the whole post chain). Every pin below was verified
with `npm view <pkg> version` / `... peerDependencies`; the lockfile must match `apps/landing/package.json`.

| Package            | Pin      | Why / peer note                                                                                                                                         |
| ------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| three              | 0.185.1  | the `~0.185` line R3F v9 + drei target                                                                                                                  |
| @react-three/fiber | 9.6.1    | pairs with React 19 (peer `react >=19 <19.3`, `three >=0.156`)                                                                                          |
| @react-three/drei  | 10.7.7   | peer `@react-three/fiber ^9.0.0`, `three >=0.159`, `react ^19`                                                                                          |
| postprocessing     | 6.39.2   | pmndrs composer; peer `three >= 0.168.0 < 0.186.0` (0.185.1 OK). See caveat below                                                                       |
| three-good-godrays | 0.12.0   | shadow-map raymarched shafts. PEER MISMATCH - see caveat, unresolved pin                                                                                |
| gsap               | 3.15.0   | ONE ScrollTrigger master timeline, `scrub:true`                                                                                                         |
| lenis              | 1.3.25   | owns document scroll; rAF driven from `gsap.ticker`                                                                                                     |
| detect-gpu         | 5.0.70   | startup quality tier (no three peer)                                                                                                                    |
| zustand            | 5.0.14   | per-frame scroll/store reads via `getState()`, never a per-frame hook selector                                                                          |
| VAT playback       | in-house | `three-vat` does NOT exist on npm. In-house shader; see VAT caveat below                                                                                |
| react / react-dom  | 19.2.7   | R3F v9 requires React 19 (peer `>=19 <19.3`)                                                                                                            |
| next               | 16.2.10  | static export (`output: 'export'`)                                                                                                                      |
| typescript         | 6.0.3    |                                                                                                                                                         |
| troika-three-text  | 0.52.4   | OPTIONAL, chrome-only (letterbox/timecode/depth-gauge labels). NOT the copy authority any more; the Semantic layer is. TTF-only, explicit `characters`. |
| framer-motion      | ^12      | DOM-only: title card / mute toggle / a11y overlay - NEVER over the canvas                                                                               |

**Pin caveats that bite:**

- **postprocessing - NEVER 6.39.0.** 6.39.0's three peer is `>= 0.168.0 < 0.184.0`, which excludes
  our three 0.185.1; a lockfile resolving 6.39.0 is a blocker. 6.39.2 (and 6.39.3, same peer
  `< 0.186.0`) fix it. Stay on 6.39.2 to match the lockfile; do not float.
- **three-good-godrays 0.12.0 - declared peer excludes our three.** Its peer is
  `three '>= 0.125.0 <= 0.182.0'` (postprocessing `^6.33.4`, satisfied). Our three 0.185.1 is
  ABOVE that hard cap, so `pnpm install` emits an unmet-peer WARNING (not an error - the repo has no
  `strict-peer-dependencies`, so it installs). This is UNVERIFIED-at-runtime until M3: the godrays
  Pass raymarches the shadow map through stable three API, so it is likely fine on 0.185, but that
  is an assumption. Resolution options, decide at M3: (a) widen the peer with a
  `pnpm-workspace.yaml` `packageExtensions` entry and verify the shafts actually render, keeping an
  inline raymarch effect as fallback; (b) if it breaks, inline the raymarch as an in-house
  postprocessing Effect (shader-engineer). Do not silently ship broken shafts.
- **VAT playback is in-house (no dep).** The package `three-vat` named in early notes does NOT
  exist on npm (404). The only real three.js VAT lib is `@floatingworld/vat3-threejs@0.1.1` (MIT,
  SideFX Labs VAT 3.0) - but it is a single unproven 0.1.1 release and its peer `three ^0.175.0`
  (caret on 0.x = `>=0.175.0 <0.176.0`) also excludes our 0.185.1. Decision: **do VAT decode
  in-house** - it is ~40 lines of deterministic vertex-shader texture sampling we must own anyway
  for the scrub contract, and it avoids a micro-dependency on the critical path. Crib the VAT 3.0
  decode math (position + normal texture layout, fluid mode) from `@floatingworld/vat3-threejs`
  (MIT) as reference; do not add it as a runtime dep. (shader-engineer owns the decode shader.)

## The playhead model

Scroll = a single global **playhead** `t` in `[0,1]`, mapped 1:1 from native scroll over a total
page length of **3,000 vh** (_tunable_, ADR envelope 2,000-4,000 vh; use `svh`/`dvh` or px
triggers, never `vh`). One idea per scroll step. Everything scroll-driven is a **pure function of
`t`**, scrub-safe both directions - no time-accumulated state driving scroll visuals.

**Scrub smoothing:** damp the raw playhead with a scrub factor of **0.6** (_tunable_, ADR range
0.5-1) plus Lenis lerp; **both smoothings are DISABLED under reduced motion / the in-page motion
toggle** (the reduced-motion path is a chapter-stepped slideshow, not a damped scrub).

**Scrub-never-hijack:** no wheel `preventDefault`, no scroll snap, no scroll-ownership override.
Native scroll maps straight to the playhead. Interactions perturb particles / play vignettes but
**never move the playhead** (determinism is load-bearing).

**The 9-scene scroll map** (playhead range copied from ADR 0016; scene `i` active when `t` is in
its range):

| #   | Scene          | playhead `t` | Locked beat                                                                            |
| --- | -------------- | ------------ | -------------------------------------------------------------------------------------- |
| 0   | Cold open      | 0.00 - 0.05  | Live monitor (FINAL_v9, 2:47 AM), chair tips past balance, title card, floor dissolves |
| 1   | The canyon     | 0.05 - 0.30  | Slow-mo backward fall down glowing rejection towers; paper storm thickens              |
| 2   | The surface    | 0.30 - 0.38  | Hits the paper ocean; the one hard beat - letterbox flexes, sound cuts, splash crown   |
| 3   | The deep       | 0.38 - 0.52  | Underwater, god-rays thin band by band; he goes limp - the saddest frame               |
| 4   | Blackout       | 0.52 - 0.58  | Near-total dark, breathing only; a single amber point of light appears below           |
| 5   | The catch      | 0.58 - 0.64  | A submersible drone catches him gently; the "Are you sure?" HUD gag                    |
| 6   | The ascent     | 0.64 - 0.85  | Axis inverts; robot carries him up; paper folds into planes in formation; he sleeps    |
| 7   | Dawn           | 0.85 - 0.95  | Surface break into flat calm at sunrise; first warm full-color frame                   |
| 8   | Finale/credits | 0.95 - 1.00  | Robot surfaces holding one red SEND button (the CTA); credits roll; creature sting     |

Per-scene local progress `sp` = `(t - lo) / (hi - lo)` clamped to `[0,1]`; scenes map their own
sub-beats off `sp`, never off wall-clock. The axis bends back up at the ascent so the SAME water is
descended (scene 3) then re-ascended (scene 6) - one world, reversible.

## Scroll rig contract

- **Lenis owns document scroll**, its rAF driven from `gsap.ticker` (not its own rAF); call
  `ScrollTrigger.update` on `lenis.on('scroll', ...)`.
- **ONE master GSAP ScrollTrigger, `scrub:true` timeline.** Absolute tweens ONLY - no `+=`
  accumulation anywhere in the scroll path (a given `t` must produce one deterministic state).
- **Scroll writes into preallocated refs / a zustand store; GL reads per frame** via
  `store.getState()` inside the loop. React never re-renders per frame; never a hook selector for a
  per-frame value.
- **Fully reversible.** Down AND rewind to the same `t` -> identical frame. This is the whole
  contract (gate-enforced).
- **The Semantic layer's scroll sections own scroll height** (see Capability gate section). One
  master timeline binds camera, baked light state, and shader uniforms so grade and blocking move
  as one.

## Character + camera scrub (one clip)

- The protagonist, the robot, and the camera spline are ONE long **Blender glTF** clip.
- Drive it with `mixer.setTime(duration * progress)` from a **damped** scrub value (fast flicks
  must not pop poses) - one source of truth, reversible for free.
- **Cross-blend by driving action weights MANUALLY** (`action.weight = ...`); **never
  `crossFadeTo`** - it assumes wall-clock and breaks under scrubbing.
- Character animation may step "on twos" at ~12 fps against the 60 fps camera; NEVER step the
  camera/physics/light on the stepped clock (stepping motion, not just the character, looks broken).

## VAT contract (splash crown)

- The **splash crown** (scene 2) is a **Houdini FLIP** sim baked to a **VAT** (SideFX Labs VAT 3.0,
  fluid mode) - position + normal textures. Playback = **sample the texture at time t** in the
  vertex shader (shader-engineer owns the decode); no CPU per-frame work.
- **Inter-frame interpolation ON** (lerp between the two nearest VAT frames) so slow scrubs read
  smooth. Deterministic + reversible: VAT frame = `round-free lerp(scene-2 sp)`, pure `f(t)`.
- **Bake budget: <= 2 MB compressed** (KTX2, _tunable_) for the crown VAT set. A single
  uncompressed VAT blows the 10 MB envelope on its own.

## Water + light

- **Bounded Gerstner water patch** (sum of ~4-6 Gerstner waves, _tunable_) + normal maps +
  **ripple-drag injection** (cursor wake written into a small displacement DataTexture). **FFT
  ocean is tier-up only** - not in the v1 WebGL2 lane.
- **God-rays via `three-good-godrays`** (shadow-map raymarched shafts) for scenes 3-4; the pass is
  scene-gated + governor-gated (see Post chain - it is a MIDDLE pass, never the last).
- **Caustics via the differential-area GLSL technique** (screen-space derivative caustic band),
  shader-engineer.
- The monitor-blue -> dawn-gold arc ships as **baked, crossfaded light states per scroll segment**
  (not dynamic PBR lights) - cheaper and reversible.

## Paper storm

- **ONE `InstancedMesh`** for the thousands of falling sheets - one draw call. Per-instance **phase
  in a DataTexture**; **bend is analytic in the vertex shader** (no CPU transforms per sheet);
  letter text from **one atlas**.
- **True cloth only for the few hero sheets** you can pluck and read (scene 1 discovery), not the
  storm.
- InstancedMesh discipline (blank/white-instance bugs): after `setColorAt()` set
  `instanceColor.needsUpdate = true`; after `setMatrixAt()` set `instanceMatrix.needsUpdate =
true`. `instanceColor` is `null` until the first `setColorAt`, and any instance you NEVER set
  renders WHITE - initialize every slot even if the storm reveals via `.count`.

## Post chain (raw postprocessing composer, filmic)

Built on the **raw** pmndrs composer. Starting pass order (_tunable_ within the milestone that
builds it; keep the LAST pass fixed - see the composer safety note):

1. `RenderPass` (scene)
2. `GodraysPass` (three-good-godrays) - scene-gated to the deep/blackout band + governor-gated.
   MIDDLE pass, may toggle.
3. `EffectPass` -> `DepthOfFieldEffect` (cinematic focus pull bound to the story subject)
4. `EffectPass` (FINAL, owns `renderToScreen`) -> merged `[ ToneMappingEffect (filmic / AgX),
VignetteEffect, NoiseEffect grain <= ~0.04 ]`

`multisampling: 4` (MSAA; SMAA fallback where MSAA is unavailable). **Letterbox** bars (with the
hand-lettered act titles / timecode / depth gauge) are **chrome, drawn in the DOM/overlay layer,
not a GL pass** - keeps caption text crisp and keeps them in the a11y tree (decision confirmable at
the chrome milestone M6). **Nothing toggles at runtime except (a) the quality governor and (b) the
scene-gated middle godrays pass.** The FINAL pass NEVER toggles.

## Budgets + quality governor

Locked envelope (ADR 0016): **<= 10 MB total shipped** (KTX2/ETC1S textures, DRACO or meshopt
geometry, procedural gradients over images, **self-hosted decoders** - a CDN-hosted decoder is a
blocker); **draw calls < 100 desktop / < 50 mobile** (probe `renderer.info.render.calls`).

**Startup tier:** `detect-gpu` picks the initial tier (tier 3 -> HIGH, 2 -> MID, 1 -> LOW; tier 0 /
no WebGL2 -> Experience gate fails, semantic fallback, GL never mounts).

**Runtime governor:** a frame-time loop with **hysteresis** - **downgrade below 45 fps**, **upgrade
above 83 fps**, with a **cooldown (~3 s, _tunable_)** between switches so it never oscillates. Turn
knobs **in this order** (ADR-locked): **pixel ratio -> post samples -> geometry density -> effect
toggles.**

Per-tier ladder (starting values, all _tunable_; the governor moves BETWEEN rungs, it does not
invent new ones):

| Tier         | dpr cap | MSAA | paper storm count | godrays steps | DOF | grain |
| ------------ | ------- | ---- | ----------------- | ------------- | --- | ----- |
| HIGH         | 2.0     | 4    | 4000              | 60            | on  | on    |
| MID          | 1.5     | 2    | 2000              | 32            | on  | on    |
| LOW          | 1.0     | SMAA | 900               | 16            | off | on    |
| MOBILE floor | 1.0     | SMAA | 600               | 12            | off | light |

Mobile below the narrow breakpoint may use the **fixed-camera variant** (ADR-allowed). drei
`PerformanceMonitor` `onDecline` is the sanctioned adaptive hook if the static rungs are not enough.
Budget-Android testing from day one (fp16 banding + dynamic-PBR cost on Adreno/Mali is where
realistic real-time dies) - verify on real low-end hardware, not just a desktop throttle.

## A11y / UX gates

- **Reduced motion / in-page motion toggle = a chapter-stepped slideshow** of stills with identical
  copy and a frozen camera. Ship the toggle IN-PAGE (OS `prefers-reduced-motion` misses many
  vestibular users); both scrub smoothings off in this mode.
- **Photosensitivity (WCAG 2.3.1):** cap luminance delta **per unit of REAL time, not scroll
  distance** - fast scrubbing can compress a slow fade into >3 Hz flashing. Clamp playhead velocity
  through the blackout -> dawn transition (scenes 4-7) so the fade can never strobe.
- **Mobile:** `svh`/`dvh` or px triggers, never `vh`; **tap-vs-swipe discrimination** on every
  touchable system; tilt parallax is an **opt-in easter egg only**.
- **Chapter dots / scene deep-links** are hash-deep-linkable (e.g. `#the-deep`) - a hash on load
  jumps the playhead to that scene's range start.
- Copy parity: every joke and link from `landing/index.html` keeps a diegetic home AND a real
  crawlable anchor in the Semantic layer; enforced by a bidirectional diff (gate step).

## Loading choreography

- **DOM-first title card is the LCP anchor** (canvas is excluded from LCP by spec). GL boots BEHIND
  it and **streams assets by story position**: canyon during the title, ocean during the fall,
  robot during the sink - the **blackout beat (scene 4) is a free preload window**.
- **Reserve canvas dimensions for CLS**; ship JSON-LD in the Semantic layer.

## Capability gate + Semantic layer + a11y overlay (structural, from ADR 0014 - still binds)

- **Experience gate to GL:** WebGL2 AND fine pointer AND width above the narrow breakpoint AND NOT
  reduced-motion (plus any pre-launch `flipped()` guard until the owner flip). Fail any -> the
  prerendered Semantic HTML runs and GL never mounts. Reduced-motion / no-WebGL2 / coarse-pointer /
  narrow visitors therefore never reach the film - the fallback holds them whole.
- **Semantic layer is the scroll-height authority.** When GL mounts it gets `visibility:hidden` +
  `inert` - **NEVER `display:none`** (that collapses scroll height and breaks the scroll rig). It
  keeps owning SEO + machine-readable copy + scroll height while hidden; it is NOT the interactive
  surface once GL runs.
- **a11y overlay is the accessible interface while GL runs.** Because copy is diegetic / in-canvas
  (opaque to the a11y tree), a **visually-hidden but focusable** DOM overlay carries accessibility:
  REAL `<a>`/`<button>` elements over each canvas hotspot (the finale SEND / CTA, the projector-slate
  menu chip - download / GitHub / privacy / creature / skip-to-end, the cold-open early-exit
  captions, the end-credits footer with all legacy + store + sponsor links + byline + foot-nav, the
  mute / "mute the guy" toggle, dialog buttons), a **skip-link first in tab order**, and an
  `aria-live` region mirroring the letterbox captions / speech-bubble text as it changes. The
  overlay is keyboard + screen-reader operable while visually hidden; the canvas stays `aria-hidden`.
  Never leave an interactive control canvas-only.

## Scrub-safety contract (the invariants a critic checks first)

- Everything scroll-driven is a pure function of `t` - **no `+=` accumulation** in the scroll path
  (GSAP tweens absolute; store writes idempotent for a given `t`; `mixer.setTime` and VAT sampling
  and manual weight blends all pure `f(t)`).
- Read scroll state per-frame via `store.getState()` inside the loop - **never a hook selector** for
  per-frame values (a selector re-renders the tree every frame).
- **A numeric `useFrame` priority disables R3F auto-render.** ONE `priority 1` `useFrame`, owned by
  the composer, drives the render; every animation `useFrame` stays at the default priority `0`.
- **InstancedMesh:** initialize every instance (unset instances render WHITE); `needsUpdate` after
  every `setColorAt`/`setMatrixAt` (see Paper storm).
- **No `window`/`document` at render scope.** Canvas trees are `'use client'` but still
  SSR-prerendered - guard any `window`/`document` access inside effects / event handlers only.

## ASCII-only source (hard rule)

Source files under `apps/landing/src/**` must be pure ASCII. Turbopack's merged source maps crash
on multi-byte characters (the rope bug). Any user-facing copy with non-ASCII glyphs lives
`\uXXXX`-escaped in `src/content/`, never inline in a component.

## postprocessing composer safety note (durable, library-general)

postprocessing's `autoRenderToScreen` statically pins `renderToScreen=true` on the array-last pass
at construction; if that pass is ever disabled the last enabled pass renders offscreen and the
canvas goes black. TERMINAL VELOCITY only toggles MIDDLE passes (godrays) and never the last, but
keep `composer.autoRenderToScreen = false` and set `renderToScreen` explicitly on the FINAL pass as
a durable safety default (learned P4, 2026-07-18, still valid post-pivot).
