---
name: webgl-gate-audit
description: Procedures for auditing apps/landing TERMINAL VELOCITY WebGL phase gates - driving the browser to exact playhead positions across the 9 scenes, screenshot discipline, scrub + VAT determinism (scrub down THEN rewind to the same playhead -> identical frame), FPS sampling at the risk scenes, draw-call + bundle-size probes vs budget, the blackout->dawn luminance-delta/strobe check at max scrub velocity, copy-parity vs landing/index.html, and reduced-motion + no-GL fallback verification. Load when running /gate or reviewing rendered GL output.
---

# apps/landing WebGL gate-audit procedures (TERMINAL VELOCITY)

The accessibility tree is blind to a canvas: screenshots, console, and performance traces are the
ONLY evidence. Never edit code from this skill - it audits rendered output only. Contract + numbers
live in `.claude/skills/webgl-standards/SKILL.md`; experience decisions in
`docs/adr/0016-terminal-velocity-scroll-film-landing.md`.

## Driving the page

Dev server: `pnpm --filter @ajh/landing dev` (http://localhost:3000). The film is canvas-only once
GL mounts; DOM/accessibility snapshots see nothing but the hidden Semantic layer + a11y overlay.

Scroll = the **playhead** `t` in `[0,1]` over ~3,000 vh. Drive it to a target `T`:
`window.scrollTo(0, (document.documentElement.scrollHeight - innerHeight) * T)`
then wait ~2 s for Lenis + scrub damping to settle before screenshotting. The **9 scenes** and their
playhead ranges (scene active when `t` is in range):

| #   | Scene          | `t` range   | sample mid-scene `T` |
| --- | -------------- | ----------- | -------------------- |
| 0   | Cold open      | 0.00 - 0.05 | 0.025                |
| 1   | The canyon     | 0.05 - 0.30 | 0.175                |
| 2   | The surface    | 0.30 - 0.38 | 0.340 (splash/VAT)   |
| 3   | The deep       | 0.38 - 0.52 | 0.450                |
| 4   | Blackout       | 0.52 - 0.58 | 0.550                |
| 5   | The catch      | 0.58 - 0.64 | 0.610                |
| 6   | The ascent     | 0.64 - 0.85 | 0.745 (ascent fold)  |
| 7   | Dawn           | 0.85 - 0.95 | 0.900                |
| 8   | Finale/credits | 0.95 - 1.00 | 0.975                |

Sample mid-scene, not on a range boundary (transition gutters). Confirm the playhead never
hijacks: no wheel `preventDefault`, no snap, native scroll maps 1:1.

## Tools

Preferred: Chrome DevTools MCP (`mcp__chrome-devtools`) - performance traces, CPU/network
throttling, console with stack traces, screenshots. Fallback if the DevTools MCP is unavailable:
the `agent-browser` CLI (`agent-browser open <url>`, `eval "<js>"`, `screenshot <path>`, `console`)
plus rAF-counter FPS sampling; mark those checks self-reported. Screenshots stay in the audit
context - report only pass/fail + evidence lines, never raw images.

## Scrub determinism (the core gate)

Everything is a pure `f(t)`, so a given playhead must produce ONE frame regardless of direction.
Pick a `T` inside a scene (not a boundary). Approach it **from below** (scroll to `T-0.02`, settle,
then `T`) and **from above** (scroll to `T+0.02`, settle, then `T`); screenshot both. The frames
MUST match - identical camera pose, character pose, light/grade state, post state. Run it on a
motion-heavy scene (the canyon fall) AND a light-transition scene (the deep). Any drift = a
time-accumulated state or a `crossFadeTo` leaked in = HIGH failure.

## Down-AND-rewind determinism

Scroll **forward through** a scene into the next, then **back** to the earlier `T`; after settle the
frame must match the first pass exactly - no leftover pose, no half-played VAT, no stuck light
state, no doubled particles. The film is reversible end to end; a scene that stays "played" on
scroll-back is a HIGH failure. Exercise it across scene 2->3 (surface into deep) and scene 6->7
(ascent into dawn - the axis inverts here, highest risk).

## VAT scrub determinism (scene 2 splash)

At the splash (`T ~= 0.34`), the crown is a VAT played by sampling at time t. Approach `T` from
below and from above (as in scrub determinism) AND forward-then-rewind: the crown geometry MUST be
frame-identical each time (VAT sampling is deterministic; inter-frame interpolation must be
symmetric). A crown that differs by approach direction, or that "sticks" at its peak on rewind,
means the VAT time is not pure `f(t)` = HIGH failure.

## FPS sampling (risk scenes first)

Sample the two heaviest scenes explicitly: the **splash/VAT** (scene 2, `T~=0.34`) and the **ascent
fold** (scene 6, `T~=0.745`, paper-planes-in-formation); then spot-check the canyon storm (scene 1).
DevTools MCP: record a trace while scrubbing the segment, read the FPS track. Fallback rAF sampler
(~2 s, scrub during it):
`new Promise(res => { let f=0; const t0=performance.now(); const loop=()=>{f++; performance.now()-t0<2000 ? requestAnimationFrame(loop) : res(Math.round(f/2));}; requestAnimationFrame(loop); })`
The governor should hold the tier steady above 45 fps (its downgrade floor); a scene that sits below
45 fps without the governor stepping down a rung is a failure. LOW tier: CPU-throttle 4x (DevTools)
and re-measure; also verify on real budget-Android where possible (fp16 banding / PBR cost).

## Draw-call probe vs budget

At each sampled scene read `renderer.info.render.calls` (expose it, or read via the R3F store in an
`eval`): **< 100 on desktop, < 50 on mobile** at every `t`. Confirm the paper storm stays ONE draw
call (a call count that scales with sheet count means the InstancedMesh path regressed). A
monotonically climbing count as you scroll means a scene's assets are never disposed.

## Bundle-size probe vs 10 MB

Build (`pnpm --filter @ajh/landing build`) and measure the shipped `apps/landing/out` payload -
total transferred (JS + KTX2 textures + DRACO/meshopt geometry + VAT textures + audio) must be
**<= 10 MB**. Confirm textures are KTX2/ETC1S, geometry is DRACO or meshopt, and the DRACO/KTX2
**decoders are self-hosted** (a CDN-hosted decoder or a single uncompressed hero/VAT texture is a
blocker). Report the byte total and the largest three assets.

## Luminance-delta / strobe check (blackout -> dawn at max scrub velocity)

The photosensitivity risk is fast scrubbing compressing a slow fade into flashing. Scrub the
blackout -> catch -> ascent -> dawn span (scenes 4-7) as FAST as the input allows and confirm the
luminance-delta clamp holds: **no more than 3 full-frame luminance flashes in any rolling second**
(frame-by-frame trace screenshots, or a flash-budget console counter if exposed). Also confirm zero
THREE/WebGL console errors at any `t`. A fade that strobes only under fast scrub = HIGH failure
(the velocity clamp is missing).

## Copy parity

Run the bidirectional copy-diff against `landing/index.html`. Every joke and link must keep a home:
the diegetic in-world copy (monitor gags, tower signage, HUD, end-credits footer) AND a real
crawlable anchor in the Semantic layer. Any missing, extra, or reworded line fails the gate.

## Reduced-motion + no-GL fallback render

- Emulate `prefers-reduced-motion: reduce` (DevTools): the page MUST render the chapter-stepped
  slideshow / prerendered Semantic HTML with identical copy and GL must NOT mount (no canvas). Also
  verify the **in-page motion toggle** produces the same stepped-slideshow fallback.
- Verify each Experience-gate condition independently falls back to the Semantic page with NO
  canvas: sub-threshold viewport (narrow), coarse pointer, and WebGL2 unavailable. Force WebGL2
  failure via `mcp__chrome-devtools__navigate_page` `initScript` stubbing
  `HTMLCanvasElement.prototype.getContext` to return null for `webgl2`, then reload (fallback: a CDP
  `Page.addScriptToEvaluateOnNewDocument` call). Same bar for every condition: Semantic page
  renders, no canvas mounts, all links + the CTA remain real anchors, scroll height intact.

## Report

One pass/fail table row per gate check with a one-line evidence note. Numbers, not adjectives
(measured FPS, `t` positions compared, draw-call count, byte total, console error count).
Screenshots never leave the audit context.
