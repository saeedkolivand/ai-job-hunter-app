---
name: webgl-gate-audit
description: Procedures for auditing apps/landing WebGL phase gates - driving the browser to exact t positions, screenshot discipline, FPS sampling, scrub-determinism checks, fried-ramp flash counting, and reduced-motion verification. Load when running /gate or reviewing rendered GL output.
---

# apps/landing WebGL gate-audit procedures

The accessibility tree is blind to a canvas: screenshots, console, and performance traces are the
ONLY evidence. Never edit code from this skill -- it audits rendered output only.

## Driving the page

Dev server: `pnpm --filter @ajh/landing dev` (http://localhost:3000). The experience is
canvas-only once GL mounts; DOM/accessibility snapshots see nothing.

Scroll to a global t in [0,1]:
`window.scrollTo(0, (document.documentElement.scrollHeight - innerHeight) * T)`
then wait ~2s for Lenis to settle before screenshotting. The 8 beats (hero, slump, descent,
deep-fried, godmode, features, testimonials, finale) map onto t sub-ranges from the journey
definition. For a mid-beat sample use t = start + 0.2*(end - start), not the exact boundary
(boundaries are transition gutters).

## Tools

Preferred: Chrome DevTools MCP (mcp__chrome-devtools) -- performance traces, CPU/network
throttling, console with stack traces, screenshots. Fallback if the DevTools MCP is unavailable:
the `agent-browser` CLI (`agent-browser open <url>`, `eval "<js>"`, `screenshot <path>`,
`console`) plus rAF-counter FPS sampling; mark those checks self-reported. Screenshots stay in the
audit context -- report only pass/fail + evidence lines, never raw images.

## FPS

DevTools MCP: record a trace while scrolling a segment; read the FPS track. Fallback rAF sampler
(~2s, scroll during it):
`new Promise(res => { let f=0; const t0=performance.now(); const loop=()=>{f++; performance.now()-t0<2000 ? requestAnimationFrame(loop) : res(Math.round(f/2));}; requestAnimationFrame(loop); })`
LOW tier: CPU-throttle 4x (DevTools) and re-measure.

## Scrub determinism

Pick a t inside a beat (not a boundary). Approach it from below (scroll to t-0.03, then t) and from
above (t+0.03, then t); screenshot both after settle. The frames MUST match -- same camera pose,
same stroke draw-on state, same post state. Repeat for one normal beat and the deep-fried beat.

## Fried-ramp flash budget

Zero THREE/WebGL console errors are allowed at any t. Across the deep-fried beat's Pass B ramp,
confirm no more than 3 full-frame flashes in any rolling second (frame-by-frame trace screenshots,
or a flash-budget console counter if one is exposed). CA / dither / glitch inside the fried window
is intentional -- the audit checks it never strobes above ~3 flashes/second, not that it is absent.

## Reduced-motion + gate

Emulate `prefers-reduced-motion: reduce` (DevTools emulation): the page MUST render the prerendered
semantic HTML and GL must NOT mount -- no canvas, no fried effects. Likewise verify a sub-threshold
viewport (width <= 900) or coarse pointer falls back to the DOM page.

## WebGL2 unavailable

Force context creation to fail with Chrome DevTools MCP: `mcp__chrome-devtools__navigate_page`'s
`initScript` param, stubbing `HTMLCanvasElement.prototype.getContext` to return null for `webgl2`,
then navigate/reload. Fallback (no DevTools MCP): a browser launch flag or equivalent CDP
`Page.addScriptToEvaluateOnNewDocument` call. The semantic page MUST still render and no canvas
may mount, same bar as the reduced-motion/narrow/coarse fallbacks above.

## Report

One pass/fail table row per gate check with a one-line evidence note. Numbers, not adjectives
(measured FPS, t positions compared, console error count). Screenshots never leave the audit
context.
