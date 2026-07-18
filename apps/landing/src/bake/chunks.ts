// Shared GLSL helper chunks for the RIPBOOK bake + material shaders. These are
// plain template strings concatenated into ShaderMaterial fragment/vertex
// bodies; every helper is ajh_-prefixed so it never collides with three's
// injected symbols (or postprocessing's built-ins in the effect passes).
//
// NOISE bundles the hashes, value noise, a domain-rotated fbm (rotating the
// domain each octave kills axis-aligned tiling artifacts), and an sRGB->linear
// decode for authoring palette colours in the linear space the bakes / lit
// materials work in. PAPER_HEIGHT is the ONE fiber-tooth height field shared by
// the albedo bake (as a tint) and the normal/height bake (as the surface the
// normal is derived from) so the light catches exactly the fibers the albedo
// shows. Include NOISE before PAPER_HEIGHT.
//
// GLSL ES 1.00 constraints honoured: texture2D (not texture), constant loop
// bounds, mat2 column-major construction.

export const NOISE = /* glsl */ `
  float ajh_hash11(float n) {
    return fract(sin(n * 12.9898) * 43758.5453123);
  }
  float ajh_hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
  }
  float ajh_vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float a = ajh_hash21(i);
    float b = ajh_hash21(i + vec2(1.0, 0.0));
    float c = ajh_hash21(i + vec2(0.0, 1.0));
    float d = ajh_hash21(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
  }
  // Column-major rotation: columns (c,s) and (-s,c) -> R * v rotates by +a.
  mat2 ajh_rot(float a) {
    float s = sin(a);
    float c = cos(a);
    return mat2(c, s, -s, c);
  }
  float ajh_fbm5(vec2 p) {
    float sum = 0.0;
    float amp = 0.5;
    float nrm = 0.0;
    for (int i = 0; i < 5; i++) {
      sum += amp * ajh_vnoise(p);
      nrm += amp;
      p = ajh_rot(0.73) * p * 2.0;
      amp *= 0.5;
    }
    return sum / nrm;
  }
  float ajh_fbm3(vec2 p) {
    float sum = 0.0;
    float amp = 0.5;
    float nrm = 0.0;
    for (int i = 0; i < 3; i++) {
      sum += amp * ajh_vnoise(p);
      nrm += amp;
      p = ajh_rot(0.53) * p * 2.02;
      amp *= 0.5;
    }
    return sum / nrm;
  }
  vec3 ajh_srgb2lin(vec3 c) {
    return pow(c, vec3(2.2));
  }
`;

export const PAPER_HEIGHT = /* glsl */ `
  // Fiber tooth + coarse tone. uv in [0,1] across the page. Fibers run along +y
  // (page height), so the tooth noise is ANISOTROPIC: high frequency across x,
  // stretched (low frequency) along y. Frequencies are tuned so a fiber cell is
  // ~4 texels at 4096^2 -> crisp (>= ~1.5 texel/px) at the closest DPR2 camera,
  // not mushy.
  float ajh_paperHeight(vec2 uv) {
    float coarse = ajh_fbm5(uv * 5.0);
    float fiber = ajh_fbm3(vec2(uv.x * 900.0, uv.y * 90.0));
    float fiber2 = ajh_vnoise(vec2(uv.x * 1550.0, uv.y * 44.0));
    float h = 0.52 + (fiber - 0.5) * 0.46 + (fiber2 - 0.5) * 0.16 + (coarse - 0.5) * 0.18;
    return clamp(h, 0.0, 1.0);
  }
`;
