// The at-load bake harness (TS-only orchestration; the real bake GLSL is
// shader-engineer's). bakeAll(renderer) runs ONCE behind the loader, before
// onReady: for each target it allocates a WebGLRenderTarget (mips + max
// anisotropy), renders a fullscreen triangle with that target's bake
// ShaderMaterial, lets three generate the mip chain, and publishes the texture
// into the `bakes` singleton by reference. The bake-time quad + materials are
// disposed once every target is baked; the render targets themselves live for
// the whole session (their textures are what the paper/ink materials sample), so
// they are intentionally NOT disposed on unmount -- see disposeBakes for the only
// full-teardown path.
//
// bakeShaders is the registry of one ShaderMaterial factory per target. The real
// procedural bake GLSL (kraft fiber albedo + roughness, height-derived normal
// map, seeded stain/smudge atlases, channel-packed pencil hatch) lives in the
// factory bodies below; shared noise/height helpers are in ./chunks. The
// harness, RT config, sequencing, and timing are unchanged from the placeholder
// version -- only the factory outputs are real now.
//
// Fullscreen-triangle contract for the bake ShaderMaterials (placeholder + real):
//   attribute vec3 position  -- already in clip space (xy in [-1,3], z ignored)
//   attribute vec2 uv        -- [0,1] across the visible target, extends to 2 on
//                               the oversized third vertex
//   the vertex shader is just: gl_Position = vec4(position.xy, 0.0, 1.0);
//   (the camera is irrelevant; frustum culling is disabled on the quad).

import {
  BufferGeometry,
  Camera,
  Float32BufferAttribute,
  LinearFilter,
  LinearMipmapLinearFilter,
  Mesh,
  NoColorSpace,
  Scene,
  ShaderMaterial,
  type WebGLRenderer,
  WebGLRenderTarget,
} from "three";

import { resolveTier, TIER_TABLE } from "@/engine/quality";

import { type BakeName, bakes } from "./bakes";
import { NOISE, PAPER_HEIGHT } from "./chunks";

// Paper bake resolution is quality-tiered (webgl-standards: HIGH 4096, LOW 2048).
// The atlases are fixed-size regardless of tier.
const PAPER_HIGH = 4096;
const PAPER_LOW = 2048;
const STAIN_SIZE = 1024;
const SMUDGE_SIZE = 512;
const HATCH_SIZE = 2048;

// Shared placeholder vertex shader for every bake target: pass the clip-space
// fullscreen triangle straight through and forward uv.
const BAKE_VERT = /* glsl */ `
  varying vec2 vUv;
  void main() {
    vUv = uv;
    gl_Position = vec4(position.xy, 0.0, 1.0);
  }
`;

// Current paper-bake resolution (tier-driven, same value bakeAll uses). Needed
// inside the normal/height factory to size the central-difference offset and to
// keep the derived normal tilt resolution-independent.
function paperSize(): number {
  return resolveTier() === TIER_TABLE.HIGH ? PAPER_HIGH : PAPER_LOW;
}

// Bump strength constant for the paper normal. Multiplied by paperSize in the
// factory so the per-texel central difference resolves to the SAME surface tilt
// at 4096 (HIGH) and 2048 (LOW) instead of flattening on the finer tier.
const NRM_K = 0.0006;

// A small ShaderMaterial honouring the fullscreen-triangle contract.
function bakeMaterial(name: string, frag: string, uniforms = {}): ShaderMaterial {
  return new ShaderMaterial({
    name,
    uniforms,
    vertexShader: BAKE_VERT,
    fragmentShader: frag,
  });
}

// paperAlbedoRough: rgb = kraft albedo (LINEAR), a = roughness. Domain-rotated
// multi-octave fbm fiber tooth (anisotropic, from the shared height field) +
// coarse blotch tone, sparse dark flecks, faint ruled lines + editor-red margin
// line, and edge darkening toward the page borders. Palette (kraft #C9A876,
// kraft-deep #A8875A, ink #17130F, editor-red #B33A2B) is sRGB->linear decoded
// here since the render target is DATA (NoColorSpace), not display-referred.
function bakeAlbedoRough(): ShaderMaterial {
  return bakeMaterial(
    "BakeAlbedoRough",
    /* glsl */ `
      ${NOISE}
      ${PAPER_HEIGHT}
      varying vec2 vUv;
      void main() {
        vec2 uv = vUv;
        float h = ajh_paperHeight(uv);

        vec3 kraft = ajh_srgb2lin(vec3(0.788, 0.659, 0.463)); // #C9A876
        vec3 kdeep = ajh_srgb2lin(vec3(0.659, 0.529, 0.353)); // #A8875A
        vec3 ink   = ajh_srgb2lin(vec3(0.090, 0.075, 0.059)); // #17130F
        vec3 red   = ajh_srgb2lin(vec3(0.702, 0.227, 0.169)); // #B33A2B

        // Base tone: coarse blotch mix, brightened where the fiber tooth rises.
        float blotch = ajh_fbm5(uv * 3.3 + 11.0);
        vec3 col = mix(kdeep, kraft, blotch);
        col *= 0.82 + h * 0.36;

        // Sparse dark flecks toward ink.
        float fleck = smoothstep(0.985, 0.995, ajh_hash21(floor(uv * vec2(420.0, 560.0))));
        col = mix(col, ink, fleck * 0.5);

        // Faint ruled lines (printed flat ink), ~26 across the page height.
        float ly = abs(fract(uv.y * 26.0) - 0.5);
        float ruled = smoothstep(0.045, 0.010, ly) * 0.06;
        col = mix(col, ink, ruled);

        // Editor-red margin line near the left edge.
        float margin = smoothstep(0.007, 0.0025, abs(uv.x - 0.12)) * 0.55;
        col = mix(col, red, margin);

        // Edge darkening toward the borders.
        float eb = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
        col *= mix(0.72, 1.0, smoothstep(0.0, 0.07, eb));

        // Roughness: grooves rougher, printed ink a touch smoother.
        float rough = clamp(0.72 + (0.5 - h) * 0.14 - margin * 0.08, 0.55, 0.9);

        gl_FragColor = vec4(col, rough);
      }
    `,
  );
}

// paperNormalHeight: rg = tangent-space normal xy, b = height, a = free. The
// normal is derived IN THE BAKE from the shared height field by central
// differences (+/- one texel), so the material gets a real normal map aligned to
// the albedo's fibers. uInvSize = one texel in uv; uNrmScale folds in paperSize
// so the tilt is resolution-independent.
function bakeNormalHeight(): ShaderMaterial {
  const size = paperSize();
  return bakeMaterial(
    "BakeNormalHeight",
    /* glsl */ `
      ${NOISE}
      ${PAPER_HEIGHT}
      uniform float uInvSize;
      uniform float uNrmScale;
      varying vec2 vUv;
      void main() {
        vec2 uv = vUv;
        float e = uInvSize;
        float hl = ajh_paperHeight(uv - vec2(e, 0.0));
        float hr = ajh_paperHeight(uv + vec2(e, 0.0));
        float hd = ajh_paperHeight(uv - vec2(0.0, e));
        float hu = ajh_paperHeight(uv + vec2(0.0, e));
        vec3 n = normalize(vec3((hl - hr) * uNrmScale, (hd - hu) * uNrmScale, 1.0));
        gl_FragColor = vec4(n.xy * 0.5 + 0.5, ajh_paperHeight(uv), 1.0);
      }
    `,
    {
      uInvSize: { value: 1 / size },
      uNrmScale: { value: NRM_K * size },
    },
  );
}

// stain: 4x4 atlas of distinct seeded coffee/ink stains. rgb = brown tint
// (linear, rim darker), a = coverage. Each cell is a coffee RING (dark outer rim
// + faint interior) or a soft BLOTCH, chosen + shaped by a per-cell hash, with a
// noise-wobbled radius so nothing is a perfect circle. The page material picks a
// cell + places it via a seed-driven transform.
function bakeStain(): ShaderMaterial {
  return bakeMaterial(
    "BakeStain",
    /* glsl */ `
      ${NOISE}
      varying vec2 vUv;
      void main() {
        vec2 grid = vUv * 4.0;
        vec2 cell = floor(grid);
        vec2 lp = fract(grid);
        float id = cell.y * 4.0 + cell.x;
        float seed = ajh_hash11(id * 3.17 + 1.0);

        vec2 c = (lp - 0.5) * 2.0;
        float ang = atan(c.y, c.x);
        float wob = (ajh_vnoise(vec2(ang * 2.0 + seed * 20.0, seed * 7.0)) - 0.5) * 0.22;
        float r = length(c) * (1.0 + wob);

        float ring = smoothstep(0.020, 0.0, abs(r - 0.72));
        float fillC = smoothstep(0.8, 0.0, r) * 0.28;
        float blot = smoothstep(0.75, 0.35, r);
        float isRing = step(0.5, seed);
        float cov = mix(blot, max(ring, fillC), isRing);
        cov *= smoothstep(1.0, 0.9, r);
        cov = clamp(cov, 0.0, 1.0);

        vec3 brown = ajh_srgb2lin(vec3(0.42, 0.28, 0.16));
        vec3 dark  = ajh_srgb2lin(vec3(0.22, 0.14, 0.08));
        vec3 tint = mix(brown, dark, ring);
        gl_FragColor = vec4(tint, cov);
      }
    `,
  );
}

// smudge: r = graphite smear density. Anisotropic fbm stretched along a slanted
// direction, thresholded into streaks and gated by a coarse patch mask so the
// smears sit in a few places rather than covering the atlas. The page material
// darkens by this.
function bakeSmudge(): ShaderMaterial {
  return bakeMaterial(
    "BakeSmudge",
    /* glsl */ `
      ${NOISE}
      varying vec2 vUv;
      void main() {
        vec2 d = ajh_rot(0.6) * vUv;
        float streak = ajh_fbm3(vec2(d.x * 7.0, d.y * 46.0));
        // NB: 'patch' is a GLSL-ES reserved word (tessellation) -> use 'mask'.
        float mask = ajh_fbm5(vUv * 2.4 + 3.0);
        float dens = clamp(smoothstep(0.45, 0.8, streak) * smoothstep(0.35, 0.7, mask), 0.0, 1.0);
        gl_FragColor = vec4(vec3(dens), 1.0);
      }
    `,
  );
}

// hatch: channel-packed pencil hatch. R = single-direction strokes, G =
// cross-hatch (two directions), B = heavy/dense scribble (three directions), A =
// blue-noise (interleaved gradient noise, two decorrelated layers -- a good
// hash-based approximation). Strokes read hand-drawn: per-stroke width + darkness
// jitter, slight waviness, broken-stroke gaps.
function bakeHatch(): ShaderMaterial {
  return bakeMaterial(
    "BakeHatch",
    /* glsl */ `
      ${NOISE}
      varying vec2 vUv;

      float ajh_strokes(vec2 uv, float ang, float freq, float dens, float seed) {
        vec2 r = ajh_rot(ang) * uv;
        float wav = (ajh_vnoise(vec2(r.y * 3.0 + seed, seed * 1.7)) - 0.5) * 0.12;
        float x = r.x * freq + wav * freq;
        float line = floor(x);
        float fx = fract(x);
        float wj = ajh_hash11(line * 1.7 + seed);
        float dj = 0.6 + 0.4 * ajh_hash11(line * 4.3 + seed + 9.0);
        float halfw = mix(0.12, 0.34, wj);
        float present = step(1.0 - dens, ajh_hash11(line * 2.9 + seed + 3.0));
        float cov = smoothstep(halfw, halfw * 0.4, abs(fx - 0.5)) * dj * present;
        float gap = smoothstep(0.25, 0.5, ajh_vnoise(vec2(r.y * 9.0 + line, seed)));
        return clamp(cov * gap, 0.0, 1.0);
      }

      float ajh_ign(vec2 p, float o) {
        return fract(52.9829189 * fract(dot(p + o, vec2(0.06711056, 0.00583715))));
      }

      void main() {
        vec2 uv = vUv;
        float single = ajh_strokes(uv, 0.7, 26.0, 0.85, 1.0);

        float crossA = ajh_strokes(uv, 0.7, 26.0, 0.80, 5.0);
        float crossB = ajh_strokes(uv, -0.7, 26.0, 0.80, 12.0);
        float crossH = clamp(max(crossA, crossB), 0.0, 1.0);

        float heavyA = ajh_strokes(uv, 0.5, 34.0, 0.95, 20.0);
        float heavyB = ajh_strokes(uv, -0.9, 34.0, 0.95, 27.0);
        float heavyC = ajh_strokes(uv, 1.9, 30.0, 0.90, 33.0);
        float heavy = clamp(heavyA + heavyB * 0.8 + heavyC * 0.7, 0.0, 1.0);

        vec2 pp = uv * 2048.0;
        float blue = fract(ajh_ign(pp, 0.0) + ajh_ign(pp, 37.0) * 0.5);

        gl_FragColor = vec4(single, crossH, heavy, blue);
      }
    `,
  );
}

// The registry the harness renders. Each factory returns a ShaderMaterial
// honouring the fullscreen-triangle contract; bakeAll allocates the target and
// generates the mip chain.
export const bakeShaders: Record<BakeName, () => ShaderMaterial> = {
  paperAlbedoRough: bakeAlbedoRough,
  paperNormalHeight: bakeNormalHeight,
  stain: bakeStain,
  smudge: bakeSmudge,
  hatch: bakeHatch,
};

interface BakeTarget {
  name: BakeName;
  size: number;
}

// Module-held render targets so the GPU-side framebuffers are not orphaned
// (three only frees them on an explicit dispose). Session-lifetime by design.
let rendered: WebGLRenderTarget[] = [];

// A screen-covering triangle in clip space with uv that hits [0,1] across the
// visible target. Bounds are the standard oversized triangle.
function fullscreenTriangle(): BufferGeometry {
  const geo = new BufferGeometry();
  geo.setAttribute(
    "position",
    new Float32BufferAttribute([-1, -1, 0, 3, -1, 0, -1, 3, 0], 3),
  );
  geo.setAttribute("uv", new Float32BufferAttribute([0, 0, 2, 0, 0, 2], 2));
  return geo;
}

// Run every bake once. Idempotent: a StrictMode double-mount (or any second
// call) sees the paper target already filled and returns immediately, so the
// bakes never run twice. Times each target and logs ONE summary line the visual
// gate reads (total target < 1.5s).
export function bakeAll(renderer: WebGLRenderer): void {
  if (bakes.paperAlbedoRough.value !== null) return;

  const paperSize = resolveTier() === TIER_TABLE.HIGH ? PAPER_HIGH : PAPER_LOW;
  const targets: BakeTarget[] = [
    { name: "paperAlbedoRough", size: paperSize },
    { name: "paperNormalHeight", size: paperSize },
    { name: "stain", size: STAIN_SIZE },
    { name: "smudge", size: SMUDGE_SIZE },
    { name: "hatch", size: HATCH_SIZE },
  ];

  const maxAniso = renderer.capabilities.getMaxAnisotropy();
  const prevTarget = renderer.getRenderTarget();

  const geo = fullscreenTriangle();
  const quad = new Mesh(geo);
  quad.frustumCulled = false; // clip-space verts: never cull the bake quad
  const scene = new Scene();
  scene.add(quad);
  const cam = new Camera(); // ignored by the vertex shader; render() needs one

  const mats: ShaderMaterial[] = [];
  const timings: string[] = [];
  const t0 = performance.now();

  // try/finally so a bake shader that throws mid-loop still restores the render
  // target and disposes the shared quad geometry + created materials (the RTs
  // stay -- session lifetime). The error is NOT swallowed: it propagates out so
  // the boot sequence can fall back to legacy instead of the loader hanging.
  try {
    for (const { name, size } of targets) {
      const s0 = performance.now();
      const rt = new WebGLRenderTarget(size, size, {
        minFilter: LinearMipmapLinearFilter,
        magFilter: LinearFilter,
        generateMipmaps: true, // three generates the mip chain at render end
        anisotropy: maxAniso,
        colorSpace: NoColorSpace,
        depthBuffer: false,
        stencilBuffer: false,
      });
      const mat = bakeShaders[name]();
      mats.push(mat);
      quad.material = mat;
      renderer.setRenderTarget(rt);
      renderer.render(scene, cam);
      bakes[name].value = rt.texture;
      rendered.push(rt);
      timings.push(`${name} ${size} ${(performance.now() - s0).toFixed(1)}ms`);
    }
  } finally {
    renderer.setRenderTarget(prevTarget);
    for (const m of mats) m.dispose();
    geo.dispose();
  }

  const total = (performance.now() - t0).toFixed(1);
  // One summary line the visual gate reads. console.warn (not .info) because the
  // shared lint config only allows warn/error and bans inline disables.
  console.warn(`[ripbook bake] ${timings.join(" | ")} | total ${total}ms`);
}

// Full teardown for the session-lifetime bakes. Not called on unmount (the
// textures are meant to persist); exposed only so a hard reset can free the GPU
// memory if one is ever wired.
export function disposeBakes(): void {
  for (const rt of rendered) rt.dispose();
  rendered = [];
  for (const key of Object.keys(bakes) as BakeName[]) bakes[key].value = null;
}
