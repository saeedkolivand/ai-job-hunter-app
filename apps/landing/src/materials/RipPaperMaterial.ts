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
// The fragment is the M2 page v2: it samples the shared kraft bake
// (bakes.paperAlbedoRough rgb=linear albedo / a=roughness, bakes.paperNormalHeight
// rg=tangent normal / b=height), perturbs the geometry normal by the fiber map,
// lights it with the scene's single directional (the hardcoded AJH_LIGHT, matching
// RipbookExperience's directionalLight) plus an ambient wrap, adds a roughness-
// modulated soft sheen, and overlays per-page seeded stains + a graphite smudge
// (bakes.stain / bakes.smudge) via uPageSeed-driven transforms. The M1 torn-edge
// raw-pulp band is kept. The bake textures are DATA (NoColorSpace) authored linear,
// so there is NO sRGB decode on sample. Helpers are ajh_-prefixed.
//
// Per-page variation follows the ONE-shared-bake rule: never a per-page bake --
// the single stain/smudge atlas is transformed per page by uPageSeed here.
//
// Uniforms (see also engine/uniforms.ts for the shared singletons):
//   uBoil       float          -- stepped boil clock; seeds the torn-edge jitter.
//   uRipP       float[9]       -- per-page rip/exit progress (shared singleton).
//   uPageIndex  int  [0..8]    -- which page this material renders (const per mat).
//   uSide       float 0|1      -- 0 = held page, 1 = free corner flap.
//   uPaper      sampler2D      -- kraft albedo (rgb linear) + roughness (a).
//   uPaperN     sampler2D      -- tangent normal (rg) + height (b) map.
//   uStain      sampler2D      -- seeded coffee/ink stain atlas (rgb tint, a cov).
//   uSmudge     sampler2D      -- seeded graphite smudge (r density).
//   uPageSeed   float [0,1)    -- per-page seed for the stain/smudge transforms.

import { DoubleSide, ShaderMaterial } from "three";

import { bakes } from "@/bake/bakes";
import { NOISE } from "@/bake/chunks";
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
  uniform sampler2D uPaper;
  uniform sampler2D uPaperN;
  uniform sampler2D uStain;
  uniform sampler2D uSmudge;
  uniform float uPageSeed;

  varying vec2 vUv;
  varying float vSeamDist;
  varying float vEdge;
  varying vec3 vWorldN;

  ${NOISE}

  const vec3  AJH_LIGHT = vec3(-0.333, 0.667, 0.667); // world light dir (upper-front)
  const float AJH_AMBIENT = 0.5;

  void main() {
    // Bake samples are DATA (NoColorSpace) authored LINEAR -> NO sRGB decode.
    vec4 ar = texture2D(uPaper, vUv);
    vec3 albedo = ar.rgb;
    float rough = ar.a;
    vec2 nxy = texture2D(uPaperN, vUv).rg * 2.0 - 1.0;
    float nz = sqrt(max(0.0, 1.0 - dot(nxy, nxy)));

    // Torn-edge raw-pulp band (kept from M1): brighter exposed fibers, pinned on
    // the actual snapped edge verts via vEdge.
    float edge = 1.0 - smoothstep(0.0, 0.04, abs(vSeamDist));
    edge = max(edge, vEdge);
    vec3 raw = ajh_srgb2lin(vec3(0.80, 0.66, 0.44));
    albedo = mix(albedo, raw, edge * 0.6);

    // Per-page stains: two taps into the shared stain atlas, each a hashed cell
    // placed + rotated by uPageSeed (never a per-page bake). Coverage stays low.
    mat2 rs = ajh_rot(uPageSeed * 6.2831);
    float idA = floor(ajh_hash11(uPageSeed * 3.1 + 1.0) * 16.0);
    vec2 cellA = vec2(mod(idA, 4.0), floor(idA / 4.0));
    vec2 lpA = clamp(rs * (vUv - vec2(0.62, 0.55)) * 1.6 + 0.5, 0.03, 0.97);
    vec4 stA = texture2D(uStain, (cellA + lpA) / 4.0);
    albedo = mix(albedo, stA.rgb, stA.a * 0.09);

    float idB = floor(ajh_hash11(uPageSeed * 5.7 + 4.0) * 16.0);
    vec2 cellB = vec2(mod(idB, 4.0), floor(idB / 4.0));
    vec2 lpB = clamp(ajh_rot(uPageSeed * 4.0 + 1.0) * (vUv - vec2(0.30, 0.28)) * 1.9 + 0.5, 0.03, 0.97);
    vec4 stB = texture2D(uStain, (cellB + lpB) / 4.0);
    albedo = mix(albedo, stB.rgb, stB.a * 0.06);

    // Graphite smudge: seed-offset density, darkens the paper slightly.
    float sm = texture2D(uSmudge, rs * vUv * 1.3 + vec2(ajh_hash11(uPageSeed + 2.0), ajh_hash11(uPageSeed + 8.0))).r;
    albedo *= 1.0 - sm * 0.10;

    // Normal-mapped light catch. Flat page tangent frame: T=+x, B=+y, N=vWorldN
    // (the peel curl already tilts vWorldN); flip on back faces (double-sided).
    vec3 N = normalize(vWorldN);
    if (!gl_FrontFacing) N = -N;
    vec3 T = normalize(vec3(1.0, 0.0, 0.0) - N * N.x);
    vec3 B = cross(N, T);
    vec3 Nw = normalize(T * nxy.x + B * nxy.y + N * nz);

    vec3 L = normalize(AJH_LIGHT);
    float lam = clamp(dot(Nw, L), 0.0, 1.0);
    float wrap = clamp(dot(Nw, L) * 0.5 + 0.5, 0.0, 1.0); // ambient wrap
    float diff = mix(wrap, lam, 0.5);
    vec3 col = albedo * (AJH_AMBIENT + (1.0 - AJH_AMBIENT) * diff);

    // Soft sheen: rougher paper -> softer, weaker highlight. View ~ +z for the
    // near-flat page (cheap, no camera vector needed).
    vec3 V = vec3(0.0, 0.0, 1.0);
    vec3 H = normalize(L + V);
    float sh = pow(clamp(dot(Nw, H), 0.0, 1.0), mix(40.0, 8.0, rough)) * (1.0 - rough) * 0.18;
    col += sh;

    gl_FragColor = vec4(col, 1.0);
  }
`;

// Per-page seed for the stain/smudge transforms. A stable golden-ratio hash of
// the page index so held + free pieces share the same stains and each page reads
// differently once more pages come online.
const PAGE_SEED = (CORNER_TEAR_PAGE * 0.61803398875) % 1;

// One program, two instances (held + free) sharing the singleton uniform objects
// BY REFERENCE -- the composer's single writer updates both for free. The bake
// holders (bakes.*) are likewise shared by reference: they are filled before the
// scene (hence this material) mounts, so the sampler is never null. uPageIndex,
// uSide, and uPageSeed are per-material constants set at creation.
export function createRipPaperMaterial(side: number): ShaderMaterial {
  return new ShaderMaterial({
    name: "RipPaperMaterial",
    side: DoubleSide,
    uniforms: {
      uBoil,
      uRipP,
      uPageIndex: { value: CORNER_TEAR_PAGE },
      uSide: { value: side },
      uPaper: bakes.paperAlbedoRough,
      uPaperN: bakes.paperNormalHeight,
      uStain: bakes.stain,
      uSmudge: bakes.smudge,
      uPageSeed: { value: PAGE_SEED },
    },
    vertexShader: VERTEX,
    fragmentShader: FRAGMENT,
  });
}
