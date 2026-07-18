// Pure svh-freeze math for the scroll rig. The scroll track is frozen to pixels
// ONCE at mount from the mount-time viewport height (never a live vh unit, which
// reflows when mobile browser chrome shows/hides and would move the playhead
// under the user's finger). NO DOM imports -- the viewport height is passed in
// so this stays unit-testable.
//
// Freeze scope: only frozenTrackHeightPx is wired into the runtime (it sets the
// Semantic layer's min-height in Experience.tsx, which is what ScrollTrigger's
// own start:0/end:"max" then measures -- GSAP owns the live scroll->target
// mapping). scrollYToPlayhead/playheadToScrollY are the direct pixel<->playhead
// converters this same frozen geometry implies; they are exercised by the tests
// below but not yet called from the rig (createScrollRig reads ScrollTrigger's
// own proxy instead, and seek() drives Lenis via ScrollTrigger.maxScroll). Wire
// them in when a caller needs a raw scrollY (e.g. a non-ScrollTrigger consumer)
// rather than duplicating this math inline.

import { clamp01 } from "./clamp";
import { SCROLL_TRACK_SVH } from "./constants";

// Frozen scroll-track height in px. 1 svh == 1% of the small-viewport height, so
// SCROLL_TRACK_SVH svh == (SCROLL_TRACK_SVH / 100) * viewportHeightPx.
export function frozenTrackHeightPx(viewportHeightPx: number): number {
  return Math.round((SCROLL_TRACK_SVH / 100) * viewportHeightPx);
}

// The scrollable distance (what maps to playhead 0..1): track height minus one
// viewport. Guarded to be non-negative.
export function scrollableRangePx(viewportHeightPx: number): number {
  const range = frozenTrackHeightPx(viewportHeightPx) - viewportHeightPx;
  return range > 0 ? range : 0;
}

// Map an absolute scrollY to playhead t in [0, 1] against the frozen viewport.
// Routed through the shared clamp01 -- a NaN scrollY (range > 0 but a bad DOM
// read) would otherwise pass every ternary comparison as false and fall
// through as NaN instead of clamping.
export function scrollYToPlayhead(scrollY: number, viewportHeightPx: number): number {
  const range = scrollableRangePx(viewportHeightPx);
  if (range <= 0) return 0;
  return clamp01(scrollY / range);
}

// Inverse: the scrollY that lands the playhead exactly on t (used to reseed the
// scroll position at a chapter start after a mode transition or a hash link).
export function playheadToScrollY(t: number, viewportHeightPx: number): number {
  const range = scrollableRangePx(viewportHeightPx);
  return clamp01(t) * range;
}
