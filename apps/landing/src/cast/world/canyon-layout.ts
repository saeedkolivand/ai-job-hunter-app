// Pure canyon math -- NO three / NO DOM imports (unit-testable in plain node).
// Everything here is a pure function of the playhead t (or a per-instance index),
// so the whole canyon is scrub-safe and reversible: the same t always yields the
// same visual state, and rewinding retraces it exactly (the ADR-0016 scrub
// contract). The .tsx components own all three objects + per-frame plumbing; this
// file owns only the deterministic numbers.

// Canyon world extent: matches the M1 camera mapping (camera.y = -t * WORLD_HEIGHT)
// so the descent stays one continuous path across every scene.
export const WORLD_HEIGHT = 240;

const TWO_PI = Math.PI * 2;

// ---- small pure helpers ---------------------------------------------------

function clamp01(v: number): number {
  if (!Number.isFinite(v)) return 0;
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

// Hermite smoothstep between edge0 and edge1.
function smoothstep(edge0: number, edge1: number, x: number): number {
  if (edge0 === edge1) return x < edge0 ? 0 : 1;
  const t = clamp01((x - edge0) / (edge1 - edge0));
  return t * t * (3 - 2 * t);
}

function mix(a: number, b: number, x: number): number {
  return a + (b - a) * x;
}

// Deterministic integer hash -> [0, 1). No Math.random, no wall-clock: the storm
// and tower layouts are frozen for the session, so a given instance index always
// lands in the same place. (integer-mix hash, fract-free -> stable across engines.)
export function hash01(n: number): number {
  let x = Math.imul(n ^ 0x9e3779b9, 0x85ebca6b) >>> 0;
  x ^= x >>> 13;
  x = Math.imul(x, 0xc2b2ae35) >>> 0;
  x ^= x >>> 16;
  return (x >>> 0) / 4294967296;
}

// ---- paper-storm density (thickens through the canyon per the ADR) --------

// Storm thickness as a pure function of t: thin at the canyon mouth, thickening
// to full by the canyon floor, then fading out approaching the surface.
//
// Fade-out window tightened (M3 review round 2 fix): the old [0.31, 0.4] window
// kept the storm at ~74% density through the MIDDLE of scene 2 (t=0.34) --
// still a dense paper tumble hiding the water plane + splash crown behind it.
// The storm's job is done by the splash; the ADR beat there is the CROWN, not
// more paper. Fade-out now completes by t=0.32 (just past the surface
// crossing), so the storm has fully cleared for the rest of scene 2.
export function stormDensity(t: number): number {
  return smoothstep(0.05, 0.26, t) * (1 - smoothstep(0.27, 0.32, t));
}

// Active instances to reveal via InstancedMesh.count for this t. Every slot is
// still initialized at build; .count only draws a leading subset.
export function stormActiveCount(t: number, maxCount: number): number {
  const n = Math.round(maxCount * stormDensity(t));
  return n < 0 ? 0 : n > maxCount ? maxCount : n;
}

// ---- camera path (one continuous descent + canyon backward-fall framing) --

// How "inside the canyon" t is (soft-edged window over scene 1 [0.05, 0.30)).
// Drives the vertigo framing, the desk-prop visibility, and the sway amplitude
// so they all fade in/out together and never pop at a scene boundary.
export function canyonActive(t: number): number {
  return smoothstep(0.04, 0.09, t) * (1 - smoothstep(0.29, 0.33, t));
}

// Straight-down descent, shared by every scene (continuous, monotonic).
export function cameraY(t: number): number {
  return -clamp01(t) * WORLD_HEIGHT;
}

// Vertigo sway across the canyon; zero outside it so other scenes read level.
export function cameraSwayX(t: number): number {
  return Math.sin(clamp01(t) * 34) * 0.6 * canyonActive(t);
}

// Pull the camera slightly closer to the walls through the canyon.
export function cameraZ(t: number): number {
  return 6 - 2 * canyonActive(t);
}

// How far above the camera the look-at target sits -- the backward-fall framing
// (he tipped off his chair face-up, so the towers stream UPWARD past him). Zero
// outside the canyon, so other scenes look level ahead.
export function cameraLookUpY(t: number): number {
  return 6 * canyonActive(t);
}

// ---- depth-graded canyon haze (sodium-orange near the top -> cold blue deep) --

// NIGHT grade (ADR-0016 "sodium-orange and cold blue city light"): both ends
// dark, saturated, and low-luminance so the haze reads as night sky, not a
// washed daylight grey-brown mid-blend. Warm end skews hard red/orange (barely
// any blue -- a sodium-vapor ember, not a tan); cold end is a saturated deep
// navy (barely any red).
//
// FOG_WARM retuned twice (M3 review): round 1 ([0.06,0.024,0.008] ->
// [0.11,0.028,0.002]) still read chocolate live -- round 1 pushed the RATIO
// further red-dominant but did not raise the overall magnitude enough, and
// R:G had drifted to ~3.9:1 (too close to pure red, not enough orange/amber
// presence to read as "ember" once gamma-corrected). Round 2: R:G brought back
// to ~3:1 (a known-good ember ratio, e.g. #FF5500) and overall magnitude more
// than doubled again so the gamma-boosted screen output lands bright enough to
// read as a saturated glow rather than a dim muddy tone. B kept low but
// nonzero (a touch of warmth, not pure red-only).
const FOG_WARM: readonly [number, number, number] = [0.24, 0.08, 0.015];
// Exported (PR #722 CodeRabbit fix): water-layout.ts's worldFog anchors its deep
// branch's SURFACE_T boundary to this SAME constant (not a re-typed copy) so the
// waterline crossing can never drift out of sync if this value is retuned again.
export const FOG_COLD: readonly [number, number, number] = [0.006, 0.014, 0.032];

// Writes the fog rgb for this t into `out` (caller-owned, so the per-frame path
// never allocates). Sodium-orange city glow up top graded to cold blue in the
// deep canyon, per the ADR lighting arc.
export function canyonFogRGB(t: number, out: [number, number, number]): void {
  const p = smoothstep(0.05, 0.3, t);
  out[0] = mix(FOG_WARM[0], FOG_COLD[0], p);
  out[1] = mix(FOG_WARM[1], FOG_COLD[1], p);
  out[2] = mix(FOG_WARM[2], FOG_COLD[2], p);
}

// ---- deterministic per-instance layouts -----------------------------------

export interface StormInstance {
  x: number;
  y: number;
  z: number;
  rotX: number;
  rotY: number;
  rotZ: number;
  scale: number;
  seed: number; // [0,1) per-sheet decorrelation (flutter phase offset, tint)
  phase: number; // [0,2PI) flutter start phase
}

// One falling sheet's static base transform + shader seeds. Deterministic: the
// camera falls THROUGH this frozen cloud, so the "storm" is camera parallax plus
// the shader's analytic flutter -- no CPU per-sheet animation.
export function stormInstance(i: number): StormInstance {
  const h = (s: number): number => hash01(i * 9301 + s * 49297 + 233);
  return {
    x: (h(1) - 0.5) * 10, // -5..5 canyon interior width
    y: -8 - h(2) * 74, // -8..-82 down the fall band
    z: (h(3) - 0.5) * 8, // -4..4 depth
    rotX: h(4) * TWO_PI,
    rotY: h(5) * TWO_PI,
    rotZ: h(6) * TWO_PI,
    scale: 0.5 + h(7) * 0.7, // 0.5..1.2
    seed: h(8),
    phase: h(9) * TWO_PI,
  };
}

export interface TowerInstance {
  x: number;
  y: number;
  z: number;
  w: number;
  h: number;
  d: number;
  seed: number;
}

// One rejection tower. Two walls (even index = left, odd = right); deeper towers
// (higher depth layer) sit further out and further back for parallax. Tall,
// overlapping boxes read as continuous canyon walls with varied silhouettes.
export function towerInstance(i: number): TowerInstance {
  const h = (s: number): number => hash01(i * 7919 + s * 104729 + 911);
  const side = i % 2 === 0 ? -1 : 1;
  const depth = h(2); // 0..1 depth layer
  return {
    x: side * (7 + h(1) * 2.5 + depth * 6), // walls set back from the interior, widening with depth
    y: -4 - h(6) * 80, // centers spread down the fall band
    z: -2 - depth * 26, // -2..-28 receding into the canyon
    w: 4 + h(4) * 5, // 4..9 wide
    h: 24 + h(3) * 46, // 24..70 tall (overlap into a wall)
    d: 4 + h(5) * 6, // 4..10 deep
    seed: h(7),
  };
}

// ---- falling desk objects (hero props, pure f(t) parallax) ----------------

interface DeskPropConfig {
  offX: number;
  offY: number; // vertical offset from the camera at the canyon mouth
  offZ: number;
  fall: number; // extra drift below the camera as t advances
  swayA: number;
  swayF: number;
  spinX: number;
  spinY: number;
  spinZ: number;
}

// Three hero props tumbling alongside the fall. Placeholder-grade silhouettes;
// art comes later. Indices map to the meshes DeskObjects builds (0 mug, 1 chair,
// 2 sticky-note cluster).
export const DESK_PROPS: readonly DeskPropConfig[] = [
  { offX: -2.2, offY: 3.0, offZ: -3, fall: 6, swayA: 0.5, swayF: 8, spinX: 4.2, spinY: 7.8, spinZ: 2.4 },
  { offX: 2.6, offY: -2.0, offZ: -4, fall: 7, swayA: 0.7, swayF: 6, spinX: 6.6, spinY: 3.6, spinZ: 5.4 },
  { offX: 0.6, offY: 6.0, offZ: -2, fall: 5, swayA: 0.4, swayF: 10, spinX: 3.0, spinY: 5.4, spinZ: 8.4 },
];

export interface DeskPropState {
  x: number;
  y: number;
  z: number;
  rx: number;
  ry: number;
  rz: number;
  visible: boolean;
}

// Writes prop i's transform for this t into `out` (caller-owned -> no per-frame
// allocation). Pure f(t): tumble + drift are functions of t, so rewinding
// un-tumbles them exactly. Visible only while the canyon framing is active.
export function writeDeskProp(i: number, t: number, out: DeskPropState): void {
  const p = DESK_PROPS[i];
  if (!p) {
    out.visible = false;
    return;
  }
  const w = canyonActive(t);
  const drift = Math.max(0, t - 0.05) * p.fall;
  out.x = p.offX + Math.sin(t * p.swayF) * p.swayA * w;
  out.y = cameraY(t) + p.offY - drift;
  out.z = p.offZ;
  out.rx = t * p.spinX;
  out.ry = t * p.spinY;
  out.rz = t * p.spinZ;
  out.visible = w > 0.001;
}
