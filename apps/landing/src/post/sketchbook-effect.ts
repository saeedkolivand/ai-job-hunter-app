// Pass A of the two-pass post pipeline (always on): one merged custom Effect
// that gives the whole frame its "living sketchbook" surface -- procedural paper
// grain + fiber (fbm hash noise, warm tint), a subtle derivative-based ink bleed
// (luminance-edge darkening -- NO neighbor-tap convolution, so this Effect needs
// no CONVOLUTION attribute and composes freely in one EffectPass), and a warm
// vignette. All animation is keyed to uBoilTime (stepped by clocks.boilTime), so
// the grain "boils" at a hand-drawn cadence instead of shimmering per-frame.
//
// Uniforms (all float): uGrain, uBleed, uVignette, uBoilTime.
// GLSL is ASCII-only (Turbopack multi-byte sourcemap crash); every helper is
// prefixed ajh_ to avoid colliding with postprocessing's injected built-ins
// (resolution, texelSize, aspect, time). blendFunction is fixed at construction
// and never swapped at runtime (a swap recompiles the whole EffectPass).

import { Effect } from "postprocessing";
import { Uniform } from "three";

// inputColor arrives already in the composer's working (linear) space -- there is
// no external texture sampled here, so no manual pow(rgb, 2.2) sRGB decode is
// needed (that rule is for textures you sample yourself).
const fragmentShader = /* glsl */ `
uniform float uGrain;
uniform float uBleed;
uniform float uVignette;
uniform float uBoilTime;

float ajh_hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float ajh_noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  float a = ajh_hash(i);
  float b = ajh_hash(i + vec2(1.0, 0.0));
  float c = ajh_hash(i + vec2(0.0, 1.0));
  float d = ajh_hash(i + vec2(1.0, 1.0));
  return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

float ajh_fbm(vec2 p) {
  float v = 0.0;
  float amp = 0.5;
  for (int i = 0; i < 4; i++) {
    v += amp * ajh_noise(p);
    p *= 2.0;
    amp *= 0.5;
  }
  return v;
}

void mainImage(const in vec4 inputColor, const in vec2 uv, out vec4 outputColor) {
  vec3 color = inputColor.rgb;

  // Ink bleed: darken luminance edges using screen-space derivatives only, so no
  // neighbor texel is sampled (derivative-based -> no CONVOLUTION attribute).
  float lum = dot(color, vec3(0.299, 0.587, 0.114));
  float edge = length(vec2(dFdx(lum), dFdy(lum)));
  color *= 1.0 - clamp(edge * uBleed * 8.0, 0.0, 0.55);

  // Paper grain + fiber, pixel-locked and animated on the stepped boil clock so
  // the surface "boils" at a hand-drawn cadence rather than per-frame shimmer.
  vec2 gp = uv * resolution;
  float fiber = ajh_fbm(gp * 0.03 + uBoilTime * 0.13);
  float grain = ajh_hash(gp + uBoilTime * 1.7) - 0.5;
  color *= mix(1.0, 0.85 + 0.30 * fiber, 0.6);
  color += grain * uGrain;

  // Warm paper tint.
  color *= vec3(1.03, 1.0, 0.94);

  // Warm vignette: fade to a warm brown toward the edges, scaled by uVignette.
  float vig = 1.0 - smoothstep(0.4, 0.8, length(uv - 0.5));
  float vigAmt = mix(1.0, vig, uVignette);
  color *= mix(vec3(1.0), vec3(0.82, 0.76, 0.68), 1.0 - vigAmt);

  outputColor = vec4(color, inputColor.a);
}
`;

export interface SketchbookOptions {
  grain?: number;
  bleed?: number;
  vignette?: number;
}

export class SketchbookEffect extends Effect {
  constructor({ grain = 0.06, bleed = 0.5, vignette = 0.35 }: SketchbookOptions = {}) {
    super("SketchbookEffect", fragmentShader, {
      uniforms: new Map<string, Uniform>([
        ["uGrain", new Uniform(grain)],
        ["uBleed", new Uniform(bleed)],
        ["uVignette", new Uniform(vignette)],
        ["uBoilTime", new Uniform(0)],
      ]),
    });
  }
}
