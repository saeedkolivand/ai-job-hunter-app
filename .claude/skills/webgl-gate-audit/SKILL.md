---
name: webgl-gate-audit
description: Procedures for auditing apps/landing RIPBOOK WebGL phase gates - driving the browser to exact t positions across the 9 pages, screenshot discipline, FPS sampling, scrub + rip-reversal determinism, draw-call probing, strobe budget, copy-parity, and reduced-motion verification. Load when running /gate or reviewing rendered GL output.
---

# apps/landing WebGL gate-audit procedures

The accessibility tree is blind to a canvas: screenshots, console, and performance traces are the
ONLY evidence. Never edit code from this skill -- it audits rendered output only.

## Driving the page

Dev server: `pnpm --filter @ajh/landing dev` (http://localhost:3000). The experience is
canvas-only once GL mounts; DOM/accessibility snapshots see nothing.

Scroll to a global t in [0,1]:
`window.scrollTo(0, (document.documentElement.scrollHeight - innerHeight) * T)`
then wait ~2s for Lenis to settle before screenshotting. There are **9 pages**; page `i` is
active for `t` in `[i/9, (i+1)/9]`, in order: 0 Cover (hinge-open), 1 Slump (corner tear),
2 DescentA (horizontal mid-rip), 3 DescentB (vertical tear), 4 Fried (crumple + toss),
5 AreYouSure (folds to a paper plane), 6 Features (diagonal tear), 7 Testimonials (perforation
zip), 8 Godmode -> back cover (signature + stamp, no rip). Within a page, `p` in `[0,0.72]`
**plays** and `p` in `[0.72,1]` **scrubs the rip**. To sample a page mid-play use
`t = i/9 + 0.4/9`; to sample its rip use `t = i/9 + 0.85/9`. Avoid exact `i/9` boundaries
(transition gutters).

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

Pick a t inside a page (not a boundary). Approach it from below (scroll to t-0.03, then t) and from
above (t+0.03, then t); screenshot both after settle. The frames MUST match -- same camera pose,
same stroke draw-on state, same post state. Test one **play** region (a normal page, `p<0.72`) and
one **rip** region (the Fried crumple page, `p>0.72`).

## Rip-reversal determinism

Scroll **into** a page's rip (`t = i/9 + 0.85/9`, so `p>0.72`) then back **below** 0.72
(`t = i/9 + 0.4/9`); after settle the page must **fully reassemble** -- no torn/missing geometry,
no leftover crumple, and the Desk pile `.count` must not double-count the page. Reversibility is the
core contract (the rig is pure `f(t)`); a page that stays torn on scroll-back is a HIGH failure.

## Draw-call probe

At each sampled page read `renderer.info.render.calls` (expose it, or read via the R3F store in an
`eval`): it must stay **< 120** at every t. Also confirm **visibility scoping** -- only the active
page +/-1 is mounted; distant pages are disposed (a monotonically climbing call count as you scroll
means a page is never disposed).

## Strobe budget

Zero THREE/WebGL console errors are allowed at any t. Generic strobe guard (the post chain never
toggles now, so this is content-agnostic): across **any** page -- including the Fried crumple and
the AreYouSure paper-plane fold -- confirm no more than 3 full-frame flashes in any rolling second
(frame-by-frame trace screenshots, or a flash-budget console counter if exposed).

## Copy parity (gate step from M3)

Run the bidirectional copy-diff script against `landing/index.html`. Every visible line of GL SDF
copy must match the semantic source 1:1; any diff (missing, extra, or reworded line) fails the gate.

## Reduced-motion + gate

Emulate `prefers-reduced-motion: reduce` (DevTools emulation): the page MUST render the prerendered
semantic HTML and GL must NOT mount -- no canvas at all. Likewise verify a sub-threshold viewport
(width <= 900), a coarse pointer, and a WebGL2-unavailable context each fall back to the legacy
semantic page with no canvas (same bar for every gate condition).

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
