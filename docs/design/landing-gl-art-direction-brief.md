# Landing GL Art Direction Brief -- "Living Sketchbook"

Grill target: aijobhunter.app full-canvas WebGL rebuild (`apps/landing`, Next 16 + R3F).
Baseline: P3, judged NOT premium. This brief is decision-by-decision; recommendations are bolded.

## 1. THE DIAGNOSIS (why P3 reads cheap vs the reference class)

- The camera never rests: it rides one uniform Catmull-Rom (`journey.ts` waypoints at flat `t=i/8`), so type is always in motion-blur territory -- ITom and Apple product pages only ever show text at a velocity plateau.
- Post + lighting are ONE global stack; every beat has the same flat mood. Active Theory v4 grades palette/exposure/prop-density per environment so scenes never feel repetitive.
- Draw-on already exists (`InkStrokes.tsx` dashed `Line2`, `dashOffset` = pure `f(t)`) but is used as ambient garnish, not the scroll verb. The reference "scroll = the drawing draws itself" beat is missing.
- Strokes are uniform-width `Line2` (`linewidth` constant per stroke) -> they read as debug lines. Every hand-drawn ref has taper/pressure/paper texture (mattdesl ribbon, ITom sketch-texture reveal).
- GL text is world-space troika (`text.ts`) parented into the scene -> it inherits camera curvature and reads pasted-on (defect 1 + 7). Obys/ITom keep type either fully diegetic or screen-locked, never half-in-world while moving.

## 2. CORE MECHANIC -- "scroll = pencil"

The pencil IS the scrubber. `dashOffset` is already `f(t)` and scrub-reversible; promote it from decor to the primary verb so line-boil is a _side effect_ of scrubbing, never ambient.

Per-beat choreography (local beat-t 0..1, drives `InkStrokes` `{t0,t1}` windows):

- 0.00-0.15 camera settles into framing; only the anchor doodle leads the camera in.
- 0.10-0.55 OUTLINES draw in a spatial wave, near-to-camera first, +/-8-15% randomized per-path offset (hand-inked, not typewriter order).
- 0.35-0.65 color/fill cross-fades in BEHIND completed strokes, lagging ~20% -- color chases line, never leads.
- 0.55-0.80 GL text strokes in last, camera near-still (legibility window).
- 0.75-1.00 camera un-pins toward next beat; drawn strokes drop to idle line-boil so past beats stay alive.
- Scroll-back: everything is `f(local-t)`, so reversing un-draws in exact mirror -- no reverse timeline needed (already true today).

Variant A -- DRAW-THEN-TRAVEL (dwell rhythm). Non-uniform `t`-spacing clusters scroll around each beat; draw completes IN the plateau, camera travels only in the spent-ink gap. Pro: max legibility, cinematic breathing, matches every ref. Con: needs the `t`-remap (Sec 3) and tight `{t0,t1}` authoring per beat. **Recommended.**

Variant B -- DRAW-WHILE-TRAVELING (continuous). Linework leads on a faster eased curve, camera lags ~20% behind so lines visibly precede the lens (Codrops SVG-map pattern). Pro: keeps unbroken journey feel, less authoring. Con: text has nowhere quiet to land -> defect 1 persists unless text goes fully screen-space. Use only for the two fast travel legs (descent shaft, godmode sky).

## 3. TEXT READABILITY

Store already carries `vel` next to `t` (`engine/store.ts`), so velocity gating is nearly free -- no new plumbing.

RECOMMENDED -- hybrid (e): dwell-eased `t` + screen-space text + velocity gate.

- `journey.ts`: add a `dwellRemap(scrollT)->journeyT` that plateaus near each `i/8` (smoothstep clusters), OR shift waypoint `t` values off the uniform grid. Camera decelerates into every beat for free; `evalPose` stays pure `f(t)`, scrub-safe.
- Text out of world-space: render beat copy as a screen-space layer (drei `Html` overlay, or troika billboarded + gated), decoupled from the camera quaternion. Kills "pasted-on" + "unreadable while moving" at once.
- Gate opacity by `journeyStore.getState().vel`: fade in only when `abs(vel) < threshold`; smoothed threshold + hysteresis so trackpad inertia never flickers.

RUNNER-UP -- screen-space pin only (b): move text to the overlay, opacity keyed to beat `t` range, skip the velocity gate. Highest raw readability, lowest cost, slightly less "earned" feel. Fallback if the dwell remap slips.

## 4. COMPOSITION FIDELITY (top 6 mismatches vs `landing/index.html`)

| #   | beat    | mismatch                                                                               | fix                                           |
| --- | ------- | -------------------------------------------------------------------------------------- | --------------------------------------------- |
| 1   | Hero    | doodle demoted to floor (`y=-4.3`), below h1 -- original is a CROWN above the headline | move to `y~=+4.3, x=0`, above kicker          |
| 2   | Hero    | read order inverted (kicker->h1->sub->doodle)                                          | restore kicker->doodle->h1->sub top-down axis |
| 3   | Slump   | 5 screencap tabs flung to `x=+/-7` orbit, unrelated to doodle footprint                | cluster tight under the doodle                |
| 4   | Slump   | doodle far below its "2:47 AM" label, dead gap                                         | raise doodle to `y~=2`, hug the label         |
| 5   | Descent | doodle buried mid-shaft (`y=-26.5`) below half the cards                               | pull up right after h2, before chips/cards    |
| 6   | Descent | swarm chips scattered both walls straddling the doodle                                 | resequence chips as one wave AFTER doodle     |

Per-beat re-composition rule (translating the original's flat vertical column to camera-facing 3D):
For each beat, treat the camera's look-target as the page's vertical axis. Place elements along the axis in original read order -- **doodle-as-crown always above its headline** -- and keep secondary props (cards, chips, tabs) in a tight tilted cluster within ~1 world-unit of the axis, never flung to frame edges. Tilt is fine (+/-2-3 deg); sprawl is not. Counters/codas stay last, smallest, axis-bottom (already correct in all beats).

## 5. PREMIUM GAP CLOSERS (ranked; cost S/M/L)

1. Text only at scroll-rest (velocity gate). What: `vel`-gated opacity + screen-space text. Why: fixes defect 1 outright, the universal ref pattern. Cost: **S** (`vel` already in store).
2. Per-beat LUT/lighting grade. What: swap a small uniform set (tint, exposure, vignette, grain) per beat off `t` in the post composer. Why: kills flat mood (defect 5); Active Theory discipline. Cost: **S-M** (one uniform block, no new passes).
3. Dwell-eased `t`-spacing. What: `dwellRemap` clustering scroll at each beat. Why: gives text its quiet window + cinematic breathing. Cost: **M** (remap + re-time `{t0,t1}` per beat).
4. Draw-on promoted to primary verb. What: tighten `{t0,t1}` to the dwell plateau, lead camera with linework. Why: the core "sketchbook draws itself" bit (defect 3). Cost: **M** (authoring, mechanism exists).
5. Per-section blocking. What: vary camera height/FOV + prop density explicitly per beat (Sec 4). Why: fixes lost rhythm + sparse world (defects 2, 6). Cost: **M-L** (re-place every scene).
6. Stroke quality upgrade (defect 4). Paths, cheapest first:
   - a) Pressure-stepped `Line2`: split each stroke into N sub-`Line2` with a sine/noise `linewidth` ramp along arc-length. Fakes taper, reuses current pipeline. Cost **S-M**.
   - b) Sketch-texture reveal a la ITom `PaintRevealMaterial`: `onBeforeCompile` noise-blend a paper/graphite texture into the stroke so edges bleed. Cost **M**.
   - c) Custom ribbon geometry (mattdesl triangle-strip) with true per-vertex width + pressure curve + UV for a brush-alpha texture. Best ceiling, real cost. Cost **L**.
     Recommend **a) now, c) only if the hero beat still reads thin after a+b**.

## 6. DECISION LIST (rule on each)

1. Core mechanic rhythm: (A) draw-then-travel dwell / (B) draw-while-traveling continuous / (C) A everywhere, B only on the 2 fast legs. **Rec: C.**
2. Text placement: (A) screen-space overlay `Html` / (B) billboarded troika in-scene / (C) keep world-space. **Rec: A.**
3. Text reveal trigger: (A) velocity gate + hysteresis / (B) beat-`t` range only / (C) both. **Rec: A.**
4. Dwell easing: (A) `dwellRemap(scrollT)` smoothstep / (B) shift waypoint `t` off the grid / (C) none, stay uniform. **Rec: A.**
5. Stroke upgrade depth: (A) pressure-stepped `Line2` / (B) + sketch-texture reveal / (C) full custom ribbon. **Rec: A now, B for hero, C deferred.**
6. Per-beat grading: (A) per-beat uniform block off `t` / (B) discrete LUT textures / (C) keep single global. **Rec: A.**
7. Composition fix scope: (A) all 6 mismatches / (B) Hero + Descent crowns only / (C) defer. **Rec: A** (the doodle-as-crown rule is the signature).
8. Draw choreography authoring: (A) hand-author `{t0,t1}` + stagger per beat / (B) derive from a single density curve / (C) leave ambient. **Rec: A** for 3-5 hero strokes, B for background doodles.
9. World density: (A) add secondary background doodles per beat (low-opacity wave) / (B) leave sparse / (C) only fried + godmode. **Rec: A**, budget-capped at 3-5 choreographed + one bg wave.
10. Line-boil scope: (A) idle boil on drawn/past beats only / (B) all strokes always / (C) off. **Rec: A** (alive-past-beats, cheap).
11. Fried beat treatment: keep the nuclear post spike as the one loud set-piece? **Rec: YES** -- restraint elsewhere makes it land (Active Theory "transitions are events").
12. Rollout order: which single change ships first to re-test the ceiling? **Rec: #1 text-at-rest + #2 grading** (both cost S, together they answer "is it premium yet" before the M/L work).

## 7. RULINGS (grill session, locked)

| #   | Decision       | Ruling                                                                                                                                                            |
| --- | -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Rhythm         | C - dwell everywhere, draw-while-traveling on the 2 fast legs (descent shaft, godmode rise)                                                                       |
| 2   | Text placement | A - screen-space DOM overlay for beat copy; diegetic display moments stay GL (amends the P1 all-text-in-GL ruling)                                                |
| 3   | Text reveal    | A - velocity gate + hysteresis                                                                                                                                    |
| 4   | Dwell impl     | A - dwellRemap(scrollT) with per-beat plateau table in journey.ts                                                                                                 |
| 5   | Strokes        | A now + B (sketch-texture) on hero doodles; C (ribbon) deferred until hero still reads thin                                                                       |
| 6   | Grading        | A - per-beat uniform block (tint/exposure/vignette/grain) crossfaded off t                                                                                        |
| 7   | Composition    | A - all 6 mismatches + the crown rule (look-target = vertical axis, original read order, doodle-as-crown, tilt not sprawl)                                        |
| 8   | Choreography   | A/B split - hand-authored {t0,t1}+stagger for 3-5 hero strokes per beat; density-curve derivation for background waves                                            |
| 9   | Density        | USER OVERRIDE: dense everywhere (not budget-capped-per-beat) - enforce via merged batches; draw budget may be raised for measured LOW-tier headroom               |
| 10  | Boil           | USER OVERRIDE: all strokes always (incl. mid-draw) - current behavior stays                                                                                       |
| 11  | Fried spike    | YES - the single loud set-piece, clean ramps, confined t-window                                                                                                   |
| 12  | Rollout        | Text-at-rest + per-beat grading ship as the FIRST art-pass PR (S-cost ceiling re-test with user screenshot judgment); then dwell remap, stroke upgrade, restaging |
