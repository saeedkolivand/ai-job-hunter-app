// Pure chrome formatters: the timecode (00:00 -> 02:40) and the depth gauge
// (meters below sea level). Both are pure functions of the playhead t so they
// scrub deterministically both directions. NO DOM imports.

import { clamp01 } from "./clamp";
import { DURATION_SECONDS } from "./constants";

function pad2(n: number): string {
  return n < 10 ? `0${n}` : `${n}`;
}

// Hermite smoothstep on [0, 1]; monotonic, with f(0) === 0 and f(1) === 1.
function smoothstep(x: number): number {
  const c = clamp01(x);
  return c * c * (3 - 2 * c);
}

// The rounded integer second count formatTimecode formats, exposed
// separately so a per-frame caller (Chrome's rAF loop) can cheaply compare
// the NUMBER before allocating the formatted string, instead of
// restringifying every frame just to throw most of the results away.
export function timecodeSeconds(t: number): number {
  return Math.round(clamp01(t) * DURATION_SECONDS);
}

// mm:ss elapsed, counting up to DURATION_SECONDS across the whole film.
export function formatTimecode(t: number): string {
  const total = timecodeSeconds(t);
  const mm = Math.floor(total / 60);
  const ss = total % 60;
  return `${pad2(mm)}:${pad2(ss)}`;
}

// Depth-gauge anchors (tunable). The world is one vertical axis: street level
// until he hits the paper ocean, down to the lightless bottom at the catch,
// then the robot carries him back up to the dawn surface break.
const SURFACE_T = 0.3; // scene 2 -- hits the paper ocean
const BOTTOM_T = 0.61; // scene 5 -- the catch, deepest point
const RESURFACE_T = 0.88; // scene 7 -- dawn surface break
const MAX_DEPTH_M = 1120; // meters at the bottom

// Meters below sea level for a playhead value. 0 above the surface and after
// the resurface; rises to MAX_DEPTH_M at the bottom. Monotonic on each leg.
export function depthMeters(t: number): number {
  const c = clamp01(t);
  if (c <= SURFACE_T || c >= RESURFACE_T) return 0;
  if (c <= BOTTOM_T) {
    return MAX_DEPTH_M * smoothstep((c - SURFACE_T) / (BOTTOM_T - SURFACE_T));
  }
  return MAX_DEPTH_M * smoothstep((RESURFACE_T - c) / (RESURFACE_T - BOTTOM_T));
}

// Display string for the depth gauge, e.g. "842 m".
export function formatDepth(t: number): string {
  return `${Math.round(depthMeters(t))} m`;
}
