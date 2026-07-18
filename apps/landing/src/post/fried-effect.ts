// Pass B's nuclear core (DEEP FRIED beat only): a custom Effect that quantizes
// the frame to a few color steps, breaks the resulting bands with a 4x4 Bayer
// ordered dither, pushes saturation, and crunches the UV with a subtle radial
// barrel warp. Every knob is a pure function of uProgress (0..1) -- the recipe
// ramps uProgress from t alone -- so at uProgress = 0 this Effect is a pixel-
// exact identity (no pop at the fried-window shoulders) and NOTHING here reads a
// time clock: the whole thing is scrub-reversible f(t) via uProgress.
//
// Uniforms:
//   uProgress : float 0..1  master fade -- 0 = passthrough, 1 = full fried
//   uBits     : float ~3-5  posterize depth; steps per channel = 2^uBits
//   uSat      : float ~1..2  saturation push (>1 extrapolates away from luma)
//
// Attributes: NONE. This is single-tap -- the barrel lives in mainUv (the library
// resamples the input ONCE at the transformed UV), never a neighbor-tap loop, so
// it declares NO EffectAttribute.CONVOLUTION and can share an EffectPass with any
// other non-convolution effect (Scanline). See recipes.ts / composer.tsx for the
// pass-split rationale (mainUv here is why it cannot sit with the CONVOLUTION CA).
//
// GLSL is ASCII-only (Turbopack multi-byte sourcemap crash). Every helper is
// ajh_ prefixed so it never collides with the library's injected built-ins
// (resolution, texelSize, aspect, time). blendFunction is fixed at construction.

import { Effect } from "postprocessing";
import { Uniform } from "three";

// Radial crunch amplitude at full progress; tiny on purpose (a CRT squeeze, not a
// fisheye). Max corner displacement is AJH_BARREL * r2max (~0.5) at uProgress 1.
const fragmentShader = /* glsl */ `
uniform float uProgress;
uniform float uBits;
uniform float uSat;

const float AJH_BARREL = 0.10;

// Recursive 2x2 -> 4x4 Bayer, closed form (no array indexing, no time). Returns a
// stable ordered-dither threshold in [0,1) keyed to integer pixel coordinates.
float ajh_bayer2(vec2 a) {
  a = floor(a);
  return fract(a.x * 0.5 + a.y * a.y * 0.75);
}

float ajh_bayer4(vec2 a) {
  return ajh_bayer2(0.5 * a) * 0.25 + ajh_bayer2(a);
}

// Barrel/pincushion UV crunch, pure f(uProgress) -- resampled once by the pass.
void mainUv(inout vec2 uv) {
  vec2 c = uv - 0.5;
  float r2 = dot(c, c);
  float k = AJH_BARREL * uProgress;
  uv = 0.5 + c * (1.0 - k * r2);
}

void mainImage(const in vec4 inputColor, const in vec2 uv, out vec4 outputColor) {
  vec3 base = inputColor.rgb;

  // Saturation push: extrapolate away from luma (uSat > 1 oversaturates).
  float luma = dot(base, vec3(0.299, 0.587, 0.114));
  vec3 col = mix(vec3(luma), base, uSat);

  // Ordered-dithered posterize: quantize each channel to 2^uBits steps, then let
  // the 4x4 Bayer offset scatter the quantization error so flat bands stipple.
  float steps = exp2(uBits);
  float d = ajh_bayer4(uv * resolution) - 0.5;
  col = floor(col * steps + 0.5 + d) / steps;
  col = clamp(col, 0.0, 1.0);

  // uProgress fades the grade in from identity so the shoulders never pop.
  vec3 outc = mix(inputColor.rgb, col, uProgress);
  outputColor = vec4(outc, inputColor.a);
}
`;

export interface FriedOptions {
  bits?: number;
  saturation?: number;
}

export class FriedEffect extends Effect {
  constructor({ bits = 4, saturation = 1.6 }: FriedOptions = {}) {
    super("FriedEffect", fragmentShader, {
      uniforms: new Map<string, Uniform>([
        ["uProgress", new Uniform(0)],
        ["uBits", new Uniform(bits)],
        ["uSat", new Uniform(saturation)],
      ]),
    });
  }
}
