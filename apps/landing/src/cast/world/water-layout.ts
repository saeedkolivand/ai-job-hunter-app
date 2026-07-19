// Pure water / deep / blackout math (M3) -- NO three / NO DOM imports, so it is
// unit-testable in plain node. Every function here is a pure function of the
// playhead t (or a per-instance index), so the whole M3 band is scrub-safe and
// reversible: the same t always yields the same visual state and rewinding
// retraces it exactly (the ADR-0016 scrub contract). The ONE deliberate
// exception is the luminance-velocity clamp, which is a real-time rate limiter
// living in engine/luminance-clamp.ts -- this file only owns its pure f(t)
// TARGET (sceneLuminance) and the tunable slew cap. The .tsx components own all
// three objects + per-frame plumbing; this file owns only the deterministic
// numbers.

import { cameraY, canyonFogRGB, FOG_COLD, hash01 } from "./canyon-layout";

const TWO_PI = Math.PI * 2;

// ---- small pure helpers (mirrors canyon-layout; kept local, not exported) ----

function clamp01(v: number): number {
  if (!Number.isFinite(v)) return 0;
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

function smoothstep(edge0: number, edge1: number, x: number): number {
  if (edge0 === edge1) return x < edge0 ? 0 : 1;
  const t = clamp01((x - edge0) / (edge1 - edge0));
  return t * t * (3 - 2 * t);
}

function mix(a: number, b: number, x: number): number {
  return a + (b - a) * x;
}

// ---- world anchors ---------------------------------------------------------

// Scene 2 lo (matches scene-resolver): the paper-ocean surface crossing.
export const SURFACE_T = 0.3;

// The paper-ocean patch extent (bounded Gerstner patch per the skill). 300 units
// square, centred on the world axis; large enough to fill the frame across the
// scene 2-3 camera range without recentering it per frame (camera x/z barely move
// below the canyon, so a fixed patch stays under the camera).
export const SURFACE_PATCH = 300;
const SURFACE_HALF = SURFACE_PATCH / 2;

// World Y of the surface plane. Sits just BELOW the camera at the crossing (the
// camera descends through cameraY(SURFACE_T) = -72), so the surface is seen from
// above during the approach then from below in the deep.
export const SURFACE_WORLD_Y = cameraY(SURFACE_T) - 2;

// ---- Gerstner water (sum of a few analytic waves; pure f(t, xz)) -----------

export interface GerstnerWave {
  readonly dx: number; // direction (normalized inside the sum)
  readonly dz: number;
  readonly amp: number; // amplitude (world units)
  readonly len: number; // wavelength (world units)
  readonly steep: number; // Gerstner sharpness Q in [0, 1]
  readonly omega: number; // angular speed vs the playhead t (t spans [0,1])
}

// Four Gerstner waves (skill envelope 4-6). Shared source of truth: the water
// vertex shader builds its GLSL const array from THIS list, so the surface the
// shader draws matches the surface the CPU floating-letter layer rides.
export const GERSTNER_WAVES: readonly GerstnerWave[] = [
  { dx: 0.92, dz: 0.39, amp: 1.35, len: 62, steep: 0.6, omega: 44 },
  { dx: -0.42, dz: 0.91, amp: 0.72, len: 33, steep: 0.52, omega: 61 },
  { dx: 0.71, dz: -0.7, amp: 0.36, len: 19, steep: 0.44, omega: 79 },
  { dx: -0.86, dz: -0.51, amp: 0.18, len: 11, steep: 0.34, omega: 98 },
];

export interface SurfacePoint {
  x: number;
  y: number;
  z: number;
  nx: number;
  ny: number;
  nz: number;
}

// Full Gerstner displacement + analytic normal for a rest particle at (x, z) at
// playhead t, written into `out` (caller-owned -> no per-frame allocation). Pure
// f(t): re-evaluating at the same t gives the identical point, so floating props
// riding the surface rewind exactly. Mirrors the water vertex shader.
export function gerstnerSurface(x: number, z: number, t: number, out: SurfacePoint): void {
  let px = x;
  let pz = z;
  let py = 0;
  let sumNx = 0;
  let sumNy = 0;
  let sumNz = 0;
  for (let i = 0; i < GERSTNER_WAVES.length; i++) {
    const w = GERSTNER_WAVES[i];
    if (!w) continue;
    const dl = Math.hypot(w.dx, w.dz) || 1;
    const dX = w.dx / dl;
    const dZ = w.dz / dl;
    const k = TWO_PI / w.len;
    const phase = k * (dX * x + dZ * z) + t * w.omega;
    const c = Math.cos(phase);
    const s = Math.sin(phase);
    px += w.steep * w.amp * dX * c;
    pz += w.steep * w.amp * dZ * c;
    py += w.amp * s;
    const wa = k * w.amp;
    sumNx += dX * wa * c;
    sumNz += dZ * wa * c;
    sumNy += w.steep * wa * s;
  }
  out.x = px;
  out.y = py;
  out.z = pz;
  const nX = -sumNx;
  const nY = 1 - sumNy;
  const nZ = -sumNz;
  const nl = Math.hypot(nX, nY, nZ) || 1;
  out.nx = nX / nl;
  out.ny = nY / nl;
  out.nz = nZ / nl;
}

// Just the surface height (y displacement) for a rest particle -- the cheap path
// where the normal is not needed.
export function gerstnerHeight(x: number, z: number, t: number): number {
  let py = 0;
  for (let i = 0; i < GERSTNER_WAVES.length; i++) {
    const w = GERSTNER_WAVES[i];
    if (!w) continue;
    const dl = Math.hypot(w.dx, w.dz) || 1;
    const dX = w.dx / dl;
    const dZ = w.dz / dl;
    const k = TWO_PI / w.len;
    py += w.amp * Math.sin(k * (dX * x + dZ * z) + t * w.omega);
  }
  return py;
}

// ---- floating letters paving the surface -----------------------------------

export interface SurfaceLetter {
  x: number; // base (rest) world x
  z: number; // base (rest) world z
  yaw: number; // flat-on-water spin about the surface normal
  scale: number;
  seed: number; // [0,1) atlas-cell + flutter decorrelation (reuses the storm shader)
  phase: number; // [0,2PI) flutter start phase
}

// One floating rejection sheet's base transform. Deterministic scatter across the
// patch; the component rides each one on the Gerstner surface per frame.
export function surfaceLetterInstance(i: number): SurfaceLetter {
  const h = (s: number): number => hash01(i * 6151 + s * 33469 + 17);
  const r = SURFACE_HALF * 0.92;
  return {
    x: (h(1) - 0.5) * 2 * r,
    z: (h(2) - 0.5) * 2 * r,
    yaw: h(3) * TWO_PI,
    scale: 0.5 + h(4) * 0.5,
    seed: h(5),
    phase: h(6) * TWO_PI,
  };
}

// ---- deep drifting papers (sparse; reuses the storm instancing pattern) ----

export interface DeepPaper {
  x: number;
  y: number;
  z: number;
  rotX: number;
  rotY: number;
  rotZ: number;
  scale: number;
  seed: number;
  phase: number;
}

// One sinking sheet in the deep band (a frozen sparse cloud the camera drifts
// through, exactly like the canyon storm -- camera parallax + the shader's
// analytic flutter, no CPU per-sheet animation).
export function deepPaperInstance(i: number): DeepPaper {
  const h = (s: number): number => hash01(i * 3187 + s * 50077 + 71);
  return {
    x: (h(1) - 0.5) * 44,
    y: SURFACE_WORLD_Y - 8 - h(2) * 74, // deep band below the surface
    z: 4 - h(3) * 44, // in front of the descending camera (looking -Z)
    rotX: h(4) * TWO_PI,
    rotY: h(5) * TWO_PI,
    rotZ: h(6) * TWO_PI,
    scale: 0.6 + h(7) * 0.8,
    seed: h(8),
    phase: h(9) * TWO_PI,
  };
}

// ---- limp figure placeholder (procedural capsule body; no character work) --

export interface FigureState {
  x: number;
  y: number;
  z: number;
  rx: number;
  ry: number;
  rz: number;
  visible: boolean;
}

// The limp protagonist sinking through the deep, written into `out`
// (caller-owned). Pure f(t): tumble + sink are functions of t, so rewinding
// un-sinks them exactly. A slow limp tumble, face-up-ish. Placeholder silhouette;
// the authored glTF clip is a later milestone.
export function writeLimpFigure(t: number, out: FigureState): void {
  const c = clamp01(t);
  const vis = smoothstep(0.36, 0.4, c) * (1 - smoothstep(0.56, 0.6, c));
  out.visible = vis > 0.01;
  if (!out.visible) return;
  const sink = smoothstep(0.34, 0.58, c);
  out.x = Math.sin(c * 5) * 1.1;
  out.y = SURFACE_WORLD_Y - 6 - sink * 60; // sinks through the deep band
  out.z = -6 + Math.sin(c * 3) * 1;
  out.rx = 0.4 + Math.sin(c * 7) * 0.25; // limp, drifting tumble
  out.ry = c * 2;
  out.rz = Math.sin(c * 4) * 0.4;
}

// ---- god-ray shafts (additive-shaft fallback; see the M3 godrays decision) --

export interface GodrayShaftDef {
  x: number;
  z: number;
  tiltX: number;
  tiltZ: number;
  len: number;
  radius: number;
}

// One additive light shaft slanting down from the surface. A handful of these
// (per-tier count) stand in for volumetric god-rays without a post composer.
//
// radius retuned (M3 review fix): the original 3-6 world-unit radius rendered as
// huge pale columns filling most of the frame at the deep's typical viewing
// distance -- the ADR beat wants "thin, dying bands", not columns. Shrunk ~6-7x.
export function godrayShaft(i: number): GodrayShaftDef {
  const h = (s: number): number => hash01(i * 2749 + s * 40961 + 5);
  return {
    x: (h(1) - 0.5) * 42,
    z: -4 - h(2) * 30,
    tiltX: -0.12 + (h(3) - 0.5) * 0.18, // near-vertical, slight lean (light from above)
    tiltZ: (h(4) - 0.5) * 0.18,
    len: 70 + h(5) * 40,
    radius: 0.35 + h(6) * 0.55, // thin, dying bands -- not columns
  };
}

// Overall shaft strength as a pure function of t: rays are strong entering the
// deep and thin band by band as he sinks, gone before the blackout (the light is
// lost by then). Drives the additive material opacity. The ramp-in aligns with
// the deep-layer entry (scene 3, t=0.38) so the shafts fade IN rather than pop on
// when the deep group becomes visible.
//
// Peak capped well below 1 (M3 review fix): the god-rays are supposed to read as
// faint, dying bands against near-black per the ADR ("the saddest frame"), not a
// bright wash that hides the limp figure -- the old uncapped strength combined
// with the additive blending and the old wide shafts overexposed the whole
// scene. Combined with the radius shrink above, this keeps the deep genuinely
// dark with the figure reading against, not lost behind, a shaft.
export function godrayStrength(t: number): number {
  const c = clamp01(t);
  return 0.3 * smoothstep(0.38, 0.43, c) * (1 - smoothstep(0.48, 0.56, c));
}

// ---- camera framing extension (scenes 2-4) ---------------------------------

// Offset ADDED to cameraY(t) to get the look-at target's Y through the surface +
// deep. Zero in the canyon (so it never fights the canyon backward-fall framing)
// and eased off in the blackout (nothing left to see). Pure f(t).
//
// REDESIGNED (M3 review fix -- "the surface has no visible water"): cameraY(t)
// is one continuous linear descent (-t * WORLD_HEIGHT) that does NOT slow down
// for the water crossing, so by mid scene-2 (t=0.34) the camera has ALREADY
// fallen ~7.6 world units below the fixed SURFACE_WORLD_Y plane. The OLD offset
// was a flat -12 (look down) the instant `inWater` ramped in (from t=0.27) --
// so at t=0.34 the camera was both below the plane AND aimed even further
// below it, pointing straight away from the paved ocean / splash crown into
// the void: nothing of the surface beat was ever in frame.
//
// Fixed by blending the look TARGET (not the camera's own position) from
// "hold near the fixed water-plane Y" -- which keeps the paved ocean + splash
// crown framed through the WHOLE crossing regardless of how far the camera's
// own Y has already fallen past that fixed Y -- to "look steeply down" only
// once solidly past the crossing, at the SAME magnitude (-12 offset once fully
// saturated) the original tuning used for the deep/blackout framing the
// coordinator already approved.
//
// intoDeep window widened 0.34-0.4 -> 0.37-0.43 (M3 review round 2 fix,
// NUMERICALLY VERIFIED, not just reasoned): the round-1 window still let the
// crown drift out of the camera's vertical half-FOV (27.5deg) by t=0.36 (a
// standalone node script reimplementing this exact math measured the
// crown-to-view-direction angle across [0.30, 0.38] at 0.005 steps -- see the
// M3 handoff log for the printed numbers). Re-run with this window: worst-case
// angle across the WHOLE of [0.30, 0.38] is 22.0deg (crown) / 17.7deg (plane),
// both comfortably inside the 27.5deg half-FOV -- the water plane + splash
// crown are PROVABLY in frame for all of scene 2, not just its middle third.
// The deep-scene regression check (same script) confirms the offset is still
// EXACTLY -12 at t=0.45+ -- the coordinator's approved deep framing is
// numerically byte-for-byte unchanged by widening this window.
export function cameraLookDownOffset(t: number): number {
  const c = clamp01(t);
  const inWater = smoothstep(0.27, 0.32, c) * (1 - smoothstep(0.54, 0.58, c));
  if (inWater <= 0) return 0;
  const intoDeep = smoothstep(0.37, 0.43, c);
  const surfaceAimY = SURFACE_WORLD_Y - 3; // just below eye level -- keeps the plane + crown centred
  const deepAimY = cameraY(c) - 12; // matches the original constant once fully saturated
  const aimY = mix(surfaceAimY, deepAimY, intoDeep);
  return (aimY - cameraY(c)) * inWater;
}

// ---- depth-graded fog for the whole descent --------------------------------

// Writes the fog rgb for this t into `out` and RETURNS the FogExp2 density. Above
// the surface it delegates to the canyon night grade; below it grades midnight
// blue -> near-black and closes the density in for the blackout, so the deep
// reads as a lightless volume rather than clear water.
//
// Cold-open density pulled back (M3 review round 2 fix): a flat 0.012 density
// from t=0 read as an opaque flat brown WALL at the cold open (t~0.02) -- the
// canyon's own density was tuned for the fall (scene 1), not the still-mostly-
// static cold-open framing (scene 0, [0, 0.05)) which needs to show RECEDING
// depth (towers fading into dark distance) rather than a uniform paint-out.
// Ramps up to the established 0.012 by t=0.05, handing off right where
// canyonActive itself starts engaging (0.04-0.09) so the transition into the
// fall reads continuous. NOTE: the cold-open's camera/framing is placeholder
// until the home-office scene exists (a later milestone) -- this only fixes the
// fog from reading as a flat wall in the meantime, not the scene's content.
//
// SURFACE_T boundary aligned exactly (PR #722 CodeRabbit fix): the deep branch's
// pDeep=0 anchor used to be a separately-tuned [0.01,0.022,0.048]@0.02 that did
// NOT match the canyon side's value at t->SURFACE_T (FOG_COLD @ the established
// 0.012 canyon density) -- a real, visible pop in both density and hue right at
// the waterline crossing. The deep branch's pDeep=0 anchor now IS FOG_COLD / the
// canyon density (not a re-typed copy -- imported from canyon-layout.ts, so the
// two sides cannot drift out of sync again), making worldFog exactly continuous
// (C0) across SURFACE_T: mix(x, ..., 0) === x at t=SURFACE_T on the deep side,
// which equals the canyon side's value in the limit t->SURFACE_T from below.
export function worldFog(t: number, out: [number, number, number]): number {
  const c = clamp01(t);
  if (c < SURFACE_T) {
    canyonFogRGB(c, out);
    return mix(0.003, 0.012, smoothstep(0, 0.05, c));
  }
  const pDeep = smoothstep(SURFACE_T, 0.52, c);
  const pBlk = smoothstep(0.52, 0.58, c);
  out[0] = mix(FOG_COLD[0], 0.001, pDeep) * (1 - pBlk);
  out[1] = mix(FOG_COLD[1], 0.004, pDeep) * (1 - pBlk);
  out[2] = mix(FOG_COLD[2], 0.01, pDeep) * (1 - 0.7 * pBlk);
  return mix(0.012, 0.075, pDeep) + pBlk * 0.03;
}

// ---- luminance target + slew cap (the WCAG 2.3.1 clamp lives in engine/) ----

// Tunable max luminance change per unit of REAL time (exposure units / second)
// the luminance-velocity clamp allows through the dark scenes. Chosen so the
// biggest blackout<->bright transition (range ~1.0) takes >= ~0.6 s, making a
// full dark-bright-dark strobe cycle < ~1 Hz -- comfortably under the WCAG 3 Hz
// limit no matter how fast the playhead is scrubbed.
export const MAX_LUMINANCE_SLEW_PER_SEC = 1.6;

// The TARGET rendered luminance (used as a global tone-mapping exposure) as a
// pure function of t: full in the canyon, dimming through the surface + deep to
// near-black in the blackout. The luminance-velocity clamp eases the APPLIED
// exposure toward this so fast scrubbing cannot strobe the transitions; AT REST
// the applied value converges here, so determinism holds.
//
// Deep-band anchors darkened (M3 review fix): the deep read overblown/bright at
// t=0.45 (partly the god-rays, fixed above, but the base exposure plateau was
// also too high) -- 0.55/0.12 lowered to 0.42/0.08 so the deep genuinely reads
// dark even with the god-rays present. The blackout's terminal 0.03 anchor is
// unchanged (already correctly near-black per the coordinator's live check).
export function sceneLuminance(t: number): number {
  const c = clamp01(t);
  if (c <= SURFACE_T) return 1;
  if (c <= 0.38) return mix(1, 0.42, smoothstep(SURFACE_T, 0.38, c));
  if (c <= 0.52) return mix(0.42, 0.08, smoothstep(0.38, 0.52, c));
  if (c <= 0.58) return mix(0.08, 0.03, smoothstep(0.52, 0.58, c));
  return 0.03; // held near-black; M4 (the catch) raises it as the amber grows
}

// ---- the one warm amber point of light appearing below (blackout) ----------

// Amber point-light intensity: 0 until the blackout, then grows (a single warm
// point appearing below and growing). Pure f(t); slow, so it needs no clamp.
export function amberIntensity(t: number): number {
  return 4 * smoothstep(0.52, 0.585, clamp01(t));
}

// World Y for the amber light -- always some distance below the descending
// camera ("appears below").
export function amberLightY(t: number): number {
  return cameraY(t) - 24;
}

// ---- scene-range visibility (which world layers render for a scene) --------

export interface WorldLayers {
  canyon: boolean;
  water: boolean;
  splash: boolean;
  deep: boolean;
  markers: boolean;
}

// Pure visibility gate keyed by the discrete scene index. Skips whole groups'
// draw calls outside their range so each segment stays within the draw-call
// budget. M3 builds scenes 2-4, so the M1 debug markers now only cover the
// not-yet-built scenes 5-8.
//
// Writes into `out` (caller-owned) -- the CanyonWorld useFrame loop preallocates
// ONE WorldLayers and reuses it every frame instead of allocating a fresh object,
// per the zero-per-frame-allocation discipline. worldLayers() below is a thin
// fresh-object wrapper over this for tests / one-off callers that don't need to
// avoid the allocation; this is the single source of truth for the logic.
export function writeWorldLayers(scene: number, out: WorldLayers): void {
  out.canyon = scene <= 2;
  out.water = scene === 2 || scene === 3; // surface from above (2) + from below (3)
  out.splash = scene === 2;
  out.deep = scene === 3 || scene === 4;
  out.markers = scene >= 5;
}

// Fresh-object convenience wrapper over writeWorldLayers -- see its doc.
export function worldLayers(scene: number): WorldLayers {
  const out: WorldLayers = { canyon: false, water: false, splash: false, deep: false, markers: false };
  writeWorldLayers(scene, out);
  return out;
}
