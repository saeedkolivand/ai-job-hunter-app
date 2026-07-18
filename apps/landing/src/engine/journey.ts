// The t-space camera journey: one continuous world the camera rides as global
// t sweeps [0,1]. Story arc, all in ONE world space:
//   hero desk (y=0) -> slump (dips down/forward) -> descent (plunges to y=-40)
//   -> fried (the bottom) -> godmode (rises to y=+20) -> features corridor
//   -> testimonials wall -> finale (back at the desk).
//
// evalPose(t) is a pure function of t (scrub-safe both directions) and writes
// into module-scoped reusable objects -- ZERO per-frame allocation. Position
// and look-target each ride their own CatmullRomCurve3; the eye quaternion is
// derived from a lookAt each frame. Segment lookup is a binary search over the
// authored waypoint t-values. hardCut waypoints snap (no interpolation into
// them) so a beat can cut rather than glide -- unused by the P1 spine (the ride
// is continuous) but wired so later phases can flag a boundary.

import * as THREE from "three";

export interface Waypoint {
  t: number;
  position: THREE.Vector3;
  look: THREE.Vector3;
  hardCut?: boolean;
}

// Authored in t-space (ascending, spanning 0..1). One waypoint per beat
// boundary threads every beat's vantage point; the godmode beat also carries an
// extra mid-beat vantage (see below) so its fast rise settles before the plateau.
export const WAYPOINTS: Waypoint[] = [
  { t: 0 / 8, position: new THREE.Vector3(0, 0, 12), look: new THREE.Vector3(0, 0, 0) },
  { t: 1 / 8, position: new THREE.Vector3(0, -3, 10), look: new THREE.Vector3(1, -4, 0) },
  // descent: tip forward off the slump lip and plunge the shaft. Steep pitch
  // (~46deg entry -> ~55deg mid) + a 22-unit vertical drop reads as a fall; the
  // look-target leads down the shaft toward the disc. Exit lands just above the
  // black hole (P4's fried beat owns the crash-through). Both neighbours still
  // frame: slump-mid look ~y=-13, fried-mid look ~y=-43.5.
  { t: 2 / 8, position: new THREE.Vector3(1.6, -14, 8.8), look: new THREE.Vector3(1, -23, 0) },
  { t: 3 / 8, position: new THREE.Vector3(0, -36, 5), look: new THREE.Vector3(0, -42.5, 0) },
  { t: 4 / 8, position: new THREE.Vector3(0, -40, 6), look: new THREE.Vector3(0, -44, 0) },
  // godmode crash-through vantage: the fried->godmode reversal is a fast rise,
  // not a glide. Without this the single 4/8->5/8 segment sweeps the look-target
  // from y=-44 to +22 across the WHOLE beat, so the scene (parked at world y=22)
  // only enters frame in the beat's last ~2% (gate: beat 5 rendered empty at
  // t~0.60). This front-loads the rise (pit -> near-plateau by t~0.544); the
  // 4.35/8->5/8 leg then holds on the payoff, framing sky/sun/guy/quote/stonks
  // across t~0.55..0.625. Safe to add: getPoint keys off (index+f)/(N-1), so
  // every other beat still samples exactly its own segment.
  { t: 4.35 / 8, position: new THREE.Vector3(0, 17, 14.3), look: new THREE.Vector3(0, 20.5, 0) },
  { t: 5 / 8, position: new THREE.Vector3(0, 20, 14), look: new THREE.Vector3(0, 22, 0) },
  { t: 6 / 8, position: new THREE.Vector3(10, 19, 10), look: new THREE.Vector3(22, 19, 0) },
  { t: 7 / 8, position: new THREE.Vector3(26, 17, 12), look: new THREE.Vector3(26, 17, 0) },
  { t: 8 / 8, position: new THREE.Vector3(0, 0, 12), look: new THREE.Vector3(0, 0, 0) },
];

const N = WAYPOINTS.length;
if (N < 2) throw new Error("journey: WAYPOINTS needs at least 2 entries");

// Curves built once. centripetal parametrization avoids the cusps/overshoot a
// uniform Catmull-Rom throws on the sharp fried->godmode reversal.
const posCurve = new THREE.CatmullRomCurve3(
  WAYPOINTS.map((w) => w.position),
  false,
  "centripetal",
);
const lookCurve = new THREE.CatmullRomCurve3(
  WAYPOINTS.map((w) => w.look),
  false,
  "centripetal",
);

// Reusable scratch -- never reallocated per frame.
const _pos = new THREE.Vector3();
const _look = new THREE.Vector3();
const _quat = new THREE.Quaternion();
const _mat = new THREE.Matrix4();
const _up = new THREE.Vector3(0, 1, 0);
const _pose = { position: _pos, quaternion: _quat };

// Binary search: largest i with WAYPOINTS[i].t <= t, clamped to [0, N-2] so
// i+1 is always a valid neighbour.
function segmentIndex(t: number): number {
  let lo = 0;
  let hi = N - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    const wp = WAYPOINTS[mid];
    if (wp && wp.t <= t) lo = mid;
    else hi = mid - 1;
  }
  return Math.min(lo, N - 2);
}

export function evalPose(t: number): { position: THREE.Vector3; quaternion: THREE.Quaternion } {
  const tc = t < 0 ? 0 : t > 1 ? 1 : t;
  const i = segmentIndex(tc);
  const a = WAYPOINTS[i];
  const b = WAYPOINTS[i + 1];
  if (!a || !b) return _pose; // segmentIndex clamps i to [0, N-2], so both are always defined

  // Local fraction inside the segment; snap to the segment start across a hard
  // cut so the pose holds then jumps at the boundary instead of gliding.
  const span = b.t - a.t;
  let f = span > 1e-6 ? (tc - a.t) / span : 0;
  if (b.hardCut) f = 0;

  // getPoint maps u in [0,1] across (N-1) segments by point index, so
  // u = (i + f) / (N - 1) samples exactly this segment at fraction f.
  const u = (i + f) / (N - 1);
  posCurve.getPoint(u, _pos);
  lookCurve.getPoint(u, _look);

  _mat.lookAt(_pos, _look, _up);
  _quat.setFromRotationMatrix(_mat);

  return _pose;
}
