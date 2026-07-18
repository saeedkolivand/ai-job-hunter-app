// The Pass B "recipe machine": one pure function, applyRecipe(t, handles), that
// writes EVERY Pass B uniform and both pass-enabled flags from the global scroll
// t ALONE. No time clock, no accumulated state -- identical t gives identical
// output, so the whole fried spike is scrub-reversible in both directions.
//
// Pass B is enabled ONLY inside [FRIED_LO, FRIED_HI] (the deep-fried t-window
// plus entry/exit shoulders). Inside it a single master ramp uFried(t) drives
// everything:
//
//   t:        0.36        0.40 ........ 0.47        0.52
//   uFried:   0  --ease-->  1  (plateau)  1  --ease-->  0
//
// a trapezoid: one smoothstep rise, a flat plateau, one smoothstep fall. Every
// uniform is a monotone scale of uFried, so across the ENTIRE window the frame
// brightens exactly ONCE and darkens exactly ONCE -- there is no oscillation
// anywhere in the mapping.
//
// STROBE INVARIANT (comfort contract -- do not break):
//   Because uFried is a single-peak monotone trapezoid and every mapped uniform
//   (bloom intensity, CA offset, fried uProgress, scanline opacity) is a monotone
//   function of it, the RECIPE introduces zero flashes of its own at ANY scroll
//   speed: the effect can never flip on/off/on -- there is no cycle in f(t) to
//   flip. The only way to see repeated flashes would be the user physically
//   scrubbing scroll back and forth many times per second, which Lenis inertia
//   smooths and which the capability gate already excludes reduced-motion users
//   from. Net: well under 3 flashes / rolling second at every scroll velocity.
//   Keep uFried monotone-up-then-monotone-down; never add a sin()/fract()/noise
//   term keyed to t here.
//
// NO-POP INVARIANT:
//   uFried is EXACTLY 0 at both shoulders (t = FRIED_LO and t = FRIED_HI). Every
//   effect is a pixel-exact identity at uFried = 0 (bloom intensity 0, CA offset
//   (0,0) -> both taps land on the same texel, fried uProgress 0 -> passthrough,
//   scanline opacity 0). So the enabled flag can flip at the shoulders with no
//   visible discontinuity. uFried is written every frame (0 outside the window)
//   so a re-enable never reads a stale mid-ramp value.

import type {
  BloomEffect,
  ChromaticAberrationEffect,
  EffectPass,
  ScanlineEffect,
} from "postprocessing";
import type { Uniform } from "three";

import type { FriedEffect } from "@/post/fried-effect";

// Window + shoulders. Enabled across [FRIED_LO, FRIED_HI]; the ramp eases 0->1
// over [FRIED_LO, RISE_HI], holds, then eases 1->0 over [FALL_LO, FRIED_HI].
const FRIED_LO = 0.36;
const RISE_HI = 0.4;
const FALL_LO = 0.47;
const FRIED_HI = 0.52;

// Peak targets for each mapped uniform (reached at uFried = 1).
const BLOOM_MAX = 5;
const CA_MAX_X = 0.004;
const CA_MAX_Y = 0.002;
const SCANLINE_MAX = 0.35;

// Effect + pass handles the composer builds once and hands to applyRecipe.
// passSketch is Pass A (always enabled); it presents to screen whenever Pass B
// is off, so the composite reaches the canvas on every non-fried beat.
export interface PassBHandles {
  passSketch: EffectPass;
  passBloom: EffectPass;
  passFried: EffectPass;
  bloom: BloomEffect;
  chromaticAberration: ChromaticAberrationEffect;
  fried: FriedEffect;
  scanline: ScanlineEffect;
}

function ajhSmoothstep(e0: number, e1: number, x: number): number {
  const u = Math.min(1, Math.max(0, (x - e0) / (e1 - e0)));
  return u * u * (3 - 2 * u);
}

// The master trapezoid ramp: 0 outside [FRIED_LO, FRIED_HI], eased shoulders,
// flat 1 across the plateau. min(rise, fall) is monotone up then monotone down.
function ajhFriedRamp(t: number): number {
  const rise = ajhSmoothstep(FRIED_LO, RISE_HI, t);
  const fall = 1 - ajhSmoothstep(FALL_LO, FRIED_HI, t);
  return Math.min(rise, fall);
}

// Pure: derive all Pass B state from t. Cheap uniform writes only -- CA's offset
// Vector2 is mutated in place (no per-frame allocation) and no blendFunction is
// ever swapped (that would recompile the pass).
export function applyRecipe(t: number, h: PassBHandles): void {
  const on = t >= FRIED_LO && t <= FRIED_HI;
  h.passBloom.enabled = on;
  h.passFried.enabled = on;

  // Present the last ENABLED pass. Inside the fried window passFried is the tail
  // of the chain; outside it, passFried is disabled and Pass A (passSketch) is
  // the final pass, so it must present or the frame never reaches the screen.
  // Both flags are guarded setters (no-op when unchanged), so this only does work
  // at the two shoulder crossings, where uFried is 0 and the swap is invisible.
  h.passFried.renderToScreen = on;
  h.passSketch.renderToScreen = !on;

  const f = ajhFriedRamp(t); // 0 outside the window, trapezoid inside
  h.bloom.intensity = f * BLOOM_MAX;
  h.chromaticAberration.offset.set(f * CA_MAX_X, f * CA_MAX_Y);
  (h.fried.uniforms.get("uProgress") as Uniform).value = f;
  h.scanline.blendMode.opacity.value = f * SCANLINE_MAX;
}
