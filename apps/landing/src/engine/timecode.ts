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
const START_ALT_M = 520; // meters above the paper ocean at t=0 (tunable)

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

// Meters ABOVE the paper ocean surface while still falling. 0 at and after
// SURFACE_T; monotonically decreasing from START_ALT_M down to 0 across the
// fall so the gauge visibly moves from the very first frame instead of reading
// 0 through the entire cold-open + canyon (a gauge frozen at 0 for 30% of the
// film fails ADR-0016's "depth IS progress" contract).
export function altitudeMeters(t: number): number {
  const c = clamp01(t);
  if (c >= SURFACE_T) return 0;
  return START_ALT_M * (1 - smoothstep(c / SURFACE_T));
}

// True while the gauge should read altitude above the surface (falling);
// false once at or below the surface, where it switches to depth-below.
export function isAltitudePhase(t: number): boolean {
  return clamp01(t) < SURFACE_T;
}

// The one continuous gauge magnitude across the whole film: altitude above the
// surface before SURFACE_T, depth below it after -- "the whole film is one
// axis, so depth IS progress" (ADR-0016), never frozen through the fall.
export function gaugeMeters(t: number): number {
  return isAltitudePhase(t) ? altitudeMeters(t) : depthMeters(t);
}

// "ALT" while falling above the surface, "DEPTH" once at/below it.
export function gaugeLabel(t: number): "ALT" | "DEPTH" {
  return isAltitudePhase(t) ? "ALT" : "DEPTH";
}

// Display string for the gauge, e.g. "ALT 412 m" / "DEPTH 842 m".
export function formatGauge(t: number): string {
  return `${gaugeLabel(t)} ${Math.round(gaugeMeters(t))} m`;
}
