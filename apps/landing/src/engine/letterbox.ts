// The splash-beat letterbox flex (chrome hook). At the surface impact (scene 2)
// the letterbox bars widen briefly then relax -- the film's one hard beat. Pure
// f(t) so it is scrub-safe both directions, read by Chrome's rAF loop and mapped
// to extra bar height. The SILENCE beat that lands with it is an audio hook
// deferred to M5 (no audio in M3); this module is the visual half only.

import { clamp01 } from "./clamp";

function smoothstep(edge0: number, edge1: number, x: number): number {
  if (edge0 === edge1) return x < edge0 ? 0 : 1;
  const t = clamp01((x - edge0) / (edge1 - edge0));
  return t * t * (3 - 2 * t);
}

// Letterbox flex amount in [0, 1]: a sharp widen right at the impact (~0.30) that
// eases back out by ~0.37. Zero everywhere outside the splash beat.
export function letterboxFlex(t: number): number {
  const c = clamp01(t);
  const rise = smoothstep(0.298, 0.315, c);
  const fall = 1 - smoothstep(0.33, 0.375, c);
  return rise * fall;
}
