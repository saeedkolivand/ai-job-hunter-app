// The M1 rip-paper material: ONE ShaderMaterial program used for BOTH the held
// page and the free corner piece, distinguished only by a uSide flag. Everything
// the vertex shader does is a pure function of the singleton uniforms (uRipP,
// uBoil) -- no time-accumulated state, no position-buffer writes -- so scrubbing
// the tear backward fully reassembles the page (at ripP = 0 every displacement
// term is exactly 0, so the free edge sits back on the seam gaplessly).
//
// The JS-vs-shader split (see .claude/scratch/ripbook-pr1.md): the composer owns
// the free piece's rigid ballistic THROW (the whole freeRef group), pure
// f(exitP). This material owns the tear-front SEPARATION GATE (progressive along
// aSeamArc) plus the near-seam PEEL BEND (cylindrical curl on the free flap, a
// few-mm lift on the held lip). Both key off the same driver: uRipP[uPageIndex]
// mirrors channels[i].exitP, so gate + throw stay in lockstep.
//
// The fragment is a PLACEHOLDER for M2 (the real kraft bake): flat kraft +
// cheap 3-octave value-noise fiber tint (screen-stable, no external textures),
// a brighter raw-pulp lightening within the torn-edge band, and a one-light
// lambert so the peel curl reads in 3D. Helper functions are ajh_-prefixed to
// avoid colliding with three's injected symbols.
//
// Uniforms (see also engine/uniforms.ts for the shared singletons):
//   uBoil       float          -- stepped boil clock; seeds the torn-edge jitter.
//   uRipP       float[9]       -- per-page rip/exit progress (shared singleton).
//   uPageIndex  int  [0..8]    -- which page this material renders (const per mat).
//   uSide       float 0|1      -- 0 = held page, 1 = free corner flap.

import { DoubleSide, ShaderMaterial } from "three";

import { CORNER_TEAR_PAGE } from "@/engine/pages";
import { uBoil, uRipP } from "@/engine/uniforms";

// Side flag values for the one shared program.
export const RIP_PAPER_SIDE = { HELD: 0, FREE: 1 } as const;

const VERTEX = /* glsl */ `
  attribute float aSeamDist;
  attribute float aSeamArc;
  attribute float aEdgeFlag;
  attribute float aNoise;

  uniform float uBoil;
  uniform float uRipP[9];
  uniform int uPageIndex;
  uniform float uSide;

  varying vec2 vUv;
  varying float vSeamDist;
  varying float vEdge;
  varying vec3 vWorldN;

  // Placeholder rig tuning (real values land with the M2 paper bake).
  const float AJH_FRONT_W = 0.15;                 // tear-front softness (arc space)
  const float AJH_CURL = 2.4;                     // peel curl curvature (1/world)
  const vec2  AJH_FLAP_DIR = vec2(0.847, 0.531);  // in-plane seam -> free corner
  const float AJH_LIP_WIDTH = 0.14;               // held-lip lift band (world)
  const float AJH_LIP_LIFT = 0.03;                // held-lip max lift (~3mm)
  const float AJH_EDGE_BOIL = 0.006;              // torn-edge jitter (<0.5% page W)

  float ajh_hash11(float n) {
    return fract(sin(n * 12.9898) * 43758.5453123);
  }

  // Read this material's page from the shared uRipP array. Indexed by a loop
  // counter (a constant-index-expression) so it compiles on GLSL ES 1.00 / ANGLE
  // even though uPageIndex is a runtime uniform.
  float ajh_ripForPage() {
    float r = 0.0;
    for (int k = 0; k < 9; k++) {
      if (k == uPageIndex) r = uRipP[k];
    }
    return r;
  }

  void main() {
    float ripP = ajh_ripForPage();

    // Tear-front gate: the front sweeps -w -> 1+w across the seam arc as ripP
    // goes 0 -> 1. g = 1 where still attached, 0 where the front has passed.
    float w = AJH_FRONT_W;
    float front = -w + ripP * (1.0 + 2.0 * w);
    float g = smoothstep(front - w, front, aSeamArc);
    float sep = 1.0 - g;

    vec3 pos = position;
    vec3 nrm = vec3(0.0, 0.0, 1.0);

    if (uSide > 0.5) {
      // FREE flap: cylindrical peel roll about the seam. Hinges at the seam
      // (s = 0) and the curl accumulates outward; gated by sep so the corner
      // peels progressively rather than all at once. Additive displacement
      // (both terms -> 0 as curvature -> 0), so ripP = 0 is exactly identity.
      float s = max(0.0, -aSeamDist);
      float kappa = AJH_CURL * ripP * sep;
      if (kappa > 1.0e-4) {
        float ks = kappa * s;
        float horiz = sin(ks) / kappa - s;      // <= 0, draws the tip back
        float lift = (1.0 - cos(ks)) / kappa;   // >= 0, lifts toward camera (+z)
        pos.xy += AJH_FLAP_DIR * horiz;
        pos.z += lift;
        nrm = normalize(cos(ks) * vec3(0.0, 0.0, 1.0) - sin(ks) * vec3(AJH_FLAP_DIR, 0.0));
      }
    } else {
      // HELD lip: a few-mm 3D lift on the torn edge, falling off with |aSeamDist|
      // and gated by the same front so the lip only rises where the tear passed.
      float prox = 1.0 - smoothstep(0.0, AJH_LIP_WIDTH, abs(aSeamDist));
      pos.z += AJH_LIP_LIFT * ripP * sep * prox;
    }

    // Torn-edge boil: tiny in-plane jitter seeded off aNoise + the stepped uBoil,
    // ramped in with ripP so the edge sits exactly on the seam at rest (the two
    // pieces' duplicated edge verts only diverge once the tear is under way).
    if (aEdgeFlag > 0.5) {
      float amp = AJH_EDGE_BOIL * clamp(ripP * 4.0, 0.0, 1.0);
      float h1 = ajh_hash11(aNoise * 91.7 + uBoil * 13.3);
      float h2 = ajh_hash11(aNoise * 47.3 + uBoil * 7.1 + 5.0);
      pos.xy += (vec2(h1, h2) - 0.5) * amp;
    }

    vUv = uv;
    vSeamDist = aSeamDist;
    vEdge = aEdgeFlag;
    vWorldN = normalize(mat3(modelMatrix) * nrm);

    gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
  }
`;

const FRAGMENT = /* glsl */ `
  varying vec2 vUv;
  varying float vSeamDist;
  varying float vEdge;
  varying vec3 vWorldN;

  // Authored in LINEAR space -- there is no texture sample to sRGB-decode here,
  // and the composer encodes the final output to sRGB, so these are the linear
  // conversions of the placeholder kraft / raw-pulp tones.
  const vec3  AJH_KRAFT = vec3(0.565, 0.413, 0.220);  // ~#c6ac81
  const vec3  AJH_RAW   = vec3(0.720, 0.550, 0.320);  // brighter torn-edge pulp
  const vec3  AJH_LIGHT = vec3(-0.333, 0.667, 0.667); // world light dir (upper-front)
  const float AJH_AMBIENT = 0.55;
  const float AJH_FIBER = 0.13;   // fine-fiber tint depth
  const float AJH_BLOTCH = 0.10;  // coarse tone variation depth

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

  float ajh_fbm(vec2 p) {
    float sum = 0.0;
    float amp = 0.5;
    for (int o = 0; o < 3; o++) {
      sum += amp * ajh_vnoise(p);
      p *= 2.0;
      amp *= 0.5;
    }
    return sum;
  }

  void main() {
    // Screen-stable fiber tint from object UVs (fixed per surface point, so it
    // does not swim as the camera or the flap moves).
    float fiber = ajh_fbm(vUv * 80.0) - 0.5;
    float blotch = ajh_fbm(vUv * 4.0) - 0.5;
    vec3 col = AJH_KRAFT * (1.0 + fiber * AJH_FIBER + blotch * AJH_BLOTCH);

    // Brighter raw pulp within the torn-edge band (seam-distance falloff, pinned
    // bright on the actual snapped edge verts via vEdge).
    float edge = 1.0 - smoothstep(0.0, 0.04, abs(vSeamDist));
    edge = max(edge, vEdge);
    col = mix(col, AJH_RAW, edge * 0.6);

    // One-light lambert so the peel curl reads; double-sided, so flip on back.
    vec3 nrm = normalize(vWorldN);
    if (!gl_FrontFacing) nrm = -nrm;
    float lam = clamp(dot(nrm, normalize(AJH_LIGHT)), 0.0, 1.0);
    col *= AJH_AMBIENT + (1.0 - AJH_AMBIENT) * lam;

    gl_FragColor = vec4(col, 1.0);
  }
`;

// One program, two instances (held + free) sharing the singleton uniform objects
// BY REFERENCE -- the composer's single writer updates both for free. uPageIndex
// and uSide are per-material constants set at creation.
export function createRipPaperMaterial(side: number): ShaderMaterial {
  return new ShaderMaterial({
    name: "RipPaperMaterial",
    side: DoubleSide,
    uniforms: {
      uBoil,
      uRipP,
      uPageIndex: { value: CORNER_TEAR_PAGE },
      uSide: { value: side },
    },
    vertexShader: VERTEX,
    fragmentShader: FRAGMENT,
  });
}
