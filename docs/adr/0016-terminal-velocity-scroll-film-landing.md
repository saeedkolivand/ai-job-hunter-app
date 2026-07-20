---
status: superseded-by-0017
supersedes: 0015
supersedes-parts-of: 0014
---

# Landing rebuilds as TERMINAL VELOCITY - a realistic CG scroll-film

## Context

Recorded from the ideation session on 2026-07-18 (owner-approved).

RIPBOOK ([ADR 0015](0015-ripbook-notebook-landing.md)) - the full-WebGL kraft-paper
notebook - was **abandoned mid-M3** on 2026-07-18. The owner judged the notebook concept
the wrong vehicle and approved a complete replacement, **TERMINAL VELOCITY**. `apps/landing`
was stripped back to a bare Next skeleton in #717; RIPBOOK's in-flight M3 branch is dead
and is **not** a baseline. This is a concept pivot, not a tuning pass - it gets a fresh ADR
and a stripped app, not an in-place mutation of 0015.

Everything **structural** in [ADR 0014](0014-landing-gl-takeover.md) still constrains this
rebuild, unchanged: the deploy stays a **Next 16 static export** on GitHub Pages; a
prerendered **Semantic layer** owns SEO, accessibility, and scroll height and stays fully
readable/crawlable when WebGL does not run; the single **Experience gate** decides whether GL
mounts; the `landing/` **Passthrough** merge and the owner-held production flip still apply.
TERMINAL VELOCITY changes the _experience_, not that machinery - same as RIPBOOK did.

## Decision

The landing becomes **TERMINAL VELOCITY**: a realistic CG **scroll-film** (~2:40) that the
visitor scrolls to watch. It retells the story of `landing/index.html` (despair -> robot ->
still unemployed) as one continuous vertical shot - a burned-out job hunter tips off his
chair at 2:47 AM and falls through a canyon of rejection towers into a paper ocean to the
lightless bottom, where a small robot finds him and carries him back up to dawn. It does
everything except press send. Operational constants (spline knots, per-tier ladders, exact
texture sizes, uniform names, foley params) live in `.claude/skills/webgl-standards/SKILL.md`
once revised for TERMINAL VELOCITY; this ADR records the decisions, not the tuning numbers.

**One-shot playhead.** Zero cuts: one camera, one continuous vertical world. Scroll IS the
**playhead**, mapped native-scroll -> timeline 0->100% (~2:40), and **fully reversible** -
scrolling up rewinds the film at any point. The axis bends back up at the ascent so the same
water is descended then re-ascended. Interactions perturb particles and play vignettes but
**never move the playhead** (determinism is load-bearing).

**The 9-scene scroll map** (scene - range - the locked beat):

| #   | Scene          | %      | Locked beat                                                                            |
| --- | -------------- | ------ | -------------------------------------------------------------------------------------- |
| 0   | Cold open      | 0-5    | Live monitor (FINAL_v9, 2:47 AM), chair tips past balance, title card, floor dissolves |
| 1   | The canyon     | 5-30   | Slow-mo backward fall down glowing rejection towers; paper storm thickens              |
| 2   | The surface    | 30-38  | Hits the paper ocean; the one hard beat - letterbox flexes, sound cuts, splash crown   |
| 3   | The deep       | 38-52  | Underwater, god-rays thin band by band; he goes limp - the saddest frame               |
| 4   | Blackout       | 52-58  | Near-total dark, breathing only; a single amber point of light appears below           |
| 5   | The catch      | 58-64  | A submersible drone catches him gently; the "Are you sure?" HUD gag                    |
| 6   | The ascent     | 64-85  | Axis inverts; robot carries him up; paper folds into planes in formation; he sleeps    |
| 7   | Dawn           | 85-95  | Surface break into flat calm at sunrise; first warm full-color frame                   |
| 8   | Finale/credits | 95-100 | Robot surfaces holding one red SEND button (the CTA); credits roll; creature sting     |

**Chrome.** No web UI. A **letterbox** frame (bars carry hand-lettered act titles/captions),
a **timecode** (00:00 -> 02:40), and a **depth gauge** (meters below sea level) - the whole
film is one axis, so depth IS progress. A persistent projector-slate menu chip lives in the
top bar (download - GitHub - privacy - the creature - skip to end) reachable at any scroll
position; the cold-open letterbox carries the two legacy early-exit captions.

**Diegetic copy parity.** Every joke and link from `landing/index.html` keeps a home, told
diegetically (no UI copy): screencap gags -> the cold-open monitor + falling objects; the
24-board swarm -> tower signage; the ATS robot -> the server floor behind tower glass; the
LinkedIn feed -> an animated billboard facade; counters -> the elevator indicator (descent) +
robot HUD (ascent); the "Are you sure?" dialog -> the robot lens HUD; testimonials /
features / the honest paragraph -> the **end-credits roll as the full footer** (all legacy
links, license line, store links, sponsor links, byline, foot-nav). Parity stays enforced
by a bidirectional diff against `landing/index.html`, and every one of those links also
exists as a real crawlable anchor in the Semantic layer.

**Interactive layer.** A film you can touch, never scrub-hijacking. The cursor is a physical
**presence** (camera parallax, papers flutter from it, an underwater ripple wake; device tilt +
touch on mobile). Every scene has at least one touchable system plus one hidden discovery
(live monitor, poke-to-flail, pluck-and-read a rejection sheet, tower-window vignettes,
splash droplet nudge, bioluminescent cursor in the deep, the fake-button HUD, plane
barrel-rolls, drag-ripples at dawn, the robot-nods SEND hover). The **SEND button at the
finale is the one real action** in the whole film - the product's thesis. Easter eggs are
preserved verbatim (konami "OFFER" flip, scroll-too-fast wind roar + protest, mute-the-guy,
console greeting).

**Sound arc.** Wind + city hum -> hard silence + sub bass at the splash/deep -> a single warm
synth at the light -> strings building on the ascent -> morning birds at dawn. Scrubbing fast
= tape whir; mute drops dialogue to letterbox captions; interactions carry their own foley
(paper snap, ripple, servo beep) spatialized to the cursor. Audio is a first-class deliverable.

**Renderer: WebGL2 lane for v1.** R3F + three + the pmndrs `postprocessing` composer already
pinned in this repo, plus `three-good-godrays` (shadow-map raymarched shafts), an in-house VAT
decode shader for baked sims (errata, verified 2026-07-18: `three-vat` does not exist on npm;
VAT playback is in-house per `.claude/skills/webgl-standards/SKILL.md`), a bounded **Gerstner**
water patch, and DataTexture GPGPU - all WebGL2-only and all proven. **TSL/WebGPU is a deliberate
tier-up experiment, not the base** (it would force rebuilding the whole post chain).

**Character + camera = ONE clip.** The protagonist, the robot, and the camera spline are one
long **Blender glTF** clip driven by `mixer.setTime(duration * progress)` - one source of
truth, reversible for free. Cross-blend by driving weights manually (never `crossFadeTo`,
which assumes wall-clock and breaks under scrubbing); feed the scrub from a damped value so
fast flicks do not pop poses. One master timeline binds camera, baked light state, and shader
uniforms together so grade and blocking move as one.

**Baked heavy sims, instanced paper.** The **splash crown** is a **Houdini FLIP** sim baked to
**VAT** (Vertex Animation Texture) and played back via an in-house VAT decode shader (no
`three-vat` package - see errata above) - scrubbing a VAT is just
sampling a texture at time t: deterministic, reversible, cheap. The **paper storm** is ONE
`InstancedMesh` (thousands of sheets, one draw call): per-instance phase in a DataTexture,
bending analytic in the vertex shader, letter text from one atlas; true cloth only for the few
hero sheets you can pluck and read. The monitor-blue -> dawn-gold lighting arc ships as
**baked, crossfaded light states** per scroll segment, not dynamic PBR lights.

**Budgets + quality governor.** Locked envelope: **<= 10 MB total shipped** (KTX2/ETC1S
textures, DRACO/meshopt geometry, procedural gradients over images, **self-hosted decoders**);
**draw calls < 100 desktop / < 50 mobile**. A **quality governor** picks a startup tier
(detect-gpu) then runs a runtime frame-time loop with **hysteresis** (downgrade below 45 fps,
upgrade above 83 fps, cooldown between), turning knobs in order: pixel ratio -> post samples ->
geometry density -> effect toggles. Per-tier ladders live in the skill.

**Scroll + a11y contract.** **Scrub, never hijack**: no wheel `preventDefault`, no scroll
snap, no ownership override - native scroll maps 1:1 to the playhead. Page length **~2,000-4,000
vh**, one idea per scroll step. **Reduced motion** = a chapter-stepped slideshow of stills with
identical copy and a frozen camera, plus an **in-page motion toggle** (OS preference misses many
vestibular users). **Photosensitivity (WCAG 2.3.1)**: cap luminance delta **per unit of real
time, not scroll distance** - fast scrubbing can compress slow fades into >3 Hz flashing; clamp
velocity through the blackout -> dawn transition. **Mobile**: `svh`/`dvh` or px triggers, never
`vh`; tap-vs-swipe discrimination; tilt parallax is an opt-in easter egg only; a fixed-camera
variant below the narrow breakpoint is allowed.

**Loading choreography.** **DOM-first title card** as the LCP anchor (canvas is excluded from
LCP by spec); GL boots behind it and streams assets by story position (canyon during the title,
ocean during the fall, robot during the sink - the blackout beat is a free preload window).
Reserve canvas dimensions for CLS; ship JSON-LD in the Semantic layer.

**Delivery.** PRs land docs -> scaffold -> M1..M6 (below) and merge autonomously through the
agent fleet; the main session orchestrates only. The **production flip** PR (pages.yml builds
`apps/landing/out`, deletes only `landing/index.html`) is **opened but held for owner approval**.

## Milestones

RIPBOOK kept its milestone map inline in the ADR; TERMINAL VELOCITY does the same. Six
milestone PR chains, each closed by `project-steward` (docs/lessons sync + `graphify update .` +
`codegraph sync`). Gate = one line of what must pass before the next milestone opens.

| M   | Scope                                                               | Gate                                                                                     |
| --- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| M1  | Scroll rig + playhead + Semantic-layer parity                       | Playhead reversible down AND rewind; copy-parity diff clean; a11y fallback whole         |
| M2  | The canyon + paper storm                                            | One-draw storm within draw-call budget; scrub-deterministic tower vignettes              |
| M3  | Water surface + splash VAT + deep/blackout                          | VAT scrub deterministic both directions; luminance-delta clamp verified through blackout |
| M4  | Robot + the catch + ascent + dawn                                   | One glTF clip scrubbed via mixer.setTime, no pose pop on fast flick; ascent reversible   |
| M5  | Interactive layer + audio                                           | Interactions never move the playhead; foley spatialized; mute/captions parity            |
| M6  | Chrome + credits + a11y + perf gates + production flip (owner-held) | <= 10 MB / draw-call / fps gates green; strobe + reduced-motion pass; flip PR held       |

## Supersessions

This ADR **fully supersedes ADR 0015 (RIPBOOK)**. Every RIPBOOK-specific ruling dies; the
0014 machinery restated above carries over. Named so the reader can find what changed:

1. **The 9-page notebook model** (0015 Decision; the "9-page p-space model" of
   `webgl-standards`). Retired. Replaced by the **9-scene scroll-film** driven by a single
   playhead - no pages, no per-page `p`-space split.
2. **The Rip / Exit system** (corner tears, crumples, paper-plane folds, the Desk pile
   progress indicator). Retired. The film has no rips; progress is the **depth gauge +
   timecode**.
3. **All copy as in-canvas troika SDF text.** Retired as a mandate. TERMINAL VELOCITY is
   realistic CG with diegetic copy in-world; the **Semantic layer** keeps the machine-readable
   and crawlable copy (0014, still active).
4. **The hand-drawn ink / boil look** (`uBoil` stepped-boil, cross-hatch, crease lines,
   Line2 fat-line ink, the kraft-paper bake, animated-on-twos ink). Retired. TERMINAL VELOCITY
   is a **PBR realistic** look with filmic post; the only "on twos" that carries over is
   character animation stepped at ~12 fps against a 60 fps camera.
5. **Procedural-only characters** (no model files). Retired. The cast is now an authored
   **Blender glTF** clip (see DCC pipeline below).

## Consequences

- **DCC pipeline ownership is a new, honest cost.** The Houdini FLIP bakes, the master Blender
  glTF clip, and the VAT textures are **authored assets, not agent-generated code** - the
  repo's GL agents write code, not DCC content. Who bakes them is an open follow-up that must be
  resolved before M3/M4 can close.
- **Asset-budget discipline is load-bearing.** The <= 10 MB envelope holds only with KTX2/ETC1S,
  DRACO/meshopt, atlas discipline, and self-hosted decoders; a single uncompressed hero texture
  or a CDN-hosted decoder blows it. Gate-enforced.
- **Scrub determinism is the whole contract.** VAT sampling, `mixer.setTime`, manual weight
  blends, and one-directional-free reversibility all exist to keep the film a pure function of
  scroll. Any time-accumulated state or `crossFadeTo` breaks rewind - covered by the gate's
  down-AND-rewind check.
- **Budget-Android testing from day one.** fp16 banding and dynamic-PBR cost on Adreno/Mali is
  where realistic real-time dies; baked light states and a fixed-camera mobile variant are the
  mitigation, verified on real low-end hardware, not just a desktop throttle.
- **Reduced-motion / no-WebGL2 / coarse-pointer / narrow visitors never reach any of this**: the
  Experience gate + Semantic layer of 0014 hold them whole.

## References

- Superseded: `docs/adr/0015-ripbook-notebook-landing.md` (RIPBOOK, fully retired).
- Still-binding machinery: `docs/adr/0014-landing-gl-takeover.md` (Next 16 static export,
  Semantic layer, Experience gate, Passthrough files, staged flip).
- Glossary: `docs/CONTEXT.md` (TERMINAL VELOCITY, scroll-film, playhead, scroll map / scenes,
  depth gauge, letterbox chrome, style frames, VAT, quality governor; RIPBOOK terms marked
  superseded).
- Implementation rules: `.claude/skills/webgl-standards/SKILL.md` (awaits its TERMINAL VELOCITY
  revision; RIPBOOK constants there are superseded by this ADR). Gate procedures:
  `.claude/skills/webgl-gate-audit/SKILL.md`.
- Look-dev ground truth: the 9 approved FLUX.2 style frames (2026-07-18), stored outside the repo.
- Copy source of truth: `landing/index.html`; app: `apps/landing`; deploy:
  `.github/workflows/pages.yml`.
