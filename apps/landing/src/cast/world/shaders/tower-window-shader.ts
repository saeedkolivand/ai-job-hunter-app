// Emissive window-grid patch for the ONE tower InstancedMesh.
//
// OWNERSHIP: webgl-author wrote this as the M2 starting point (tower layout +
// wiring live in TowerCanyon.tsx). shader-engineer owns the interior "life".
// This pass adds subtle per-window life on top of the static grid: a seeded slow
// flicker (a few windows change state as the playhead advances) and a slight
// per-window warm/cool variance around uWinColor. Silhouettes (desk/figure) are
// still deferred -- kept cheap per the M2 scope.
//
// The grid is derived from object-space position + normal (no dependence on a uv
// attribute), so it works on the raw unit BoxGeometry the mesh instances. Per
// tower variation rides the aTowerSeed InstancedBufferAttribute; per-window
// variation rides a stable hash of the window cell id. The flicker is a PURE
// function of the playhead uniform uTowerT (never a wall-clock), so scrubbing is
// reversible and it never strobes (slow rates, smoothstep transitions).

import { Color, type MeshStandardMaterial } from "three";

export interface TowerUniforms {
  uWinColor: { value: Color };
  uWinIntensity: { value: number };
  uTowerT: { value: number };
}

const VERTEX_PARS = /* glsl */ `
  attribute float aTowerSeed;
  varying float vTowerSeed;
  varying vec3 vObjPos;
  varying vec3 vObjNormal;
`;

const VERTEX_BODY = /* glsl */ `
  vTowerSeed = aTowerSeed;
  vObjPos = position;   // unit box -> [-0.5, 0.5] per axis
  vObjNormal = normal;
`;

const FRAGMENT_PARS = /* glsl */ `
  uniform vec3 uWinColor;
  uniform float uWinIntensity;
  uniform float uTowerT; // playhead t in [0,1] -- the ONLY animation clock
  varying float vTowerSeed;
  varying vec3 vObjPos;
  varying vec3 vObjNormal;

  // cheap stable hash -> [0,1)
  float hashT(float n) {
    return fract(sin(n * 43758.5453123) * 12345.6789);
  }
`;

// Injected right after `#include <emissivemap_fragment>` (totalEmissiveRadiance
// exists). Side faces only -- top/bottom are masked via the object normal.
const FRAGMENT_BODY = /* glsl */ `
  float side = step(abs(vObjNormal.y), 0.5);

  // pick the two in-plane axes for the current face, unit box -> [0,1]
  vec2 face = abs(vObjNormal.x) > 0.5
    ? vec2(vObjPos.z, vObjPos.y)
    : vec2(vObjPos.x, vObjPos.y);
  vec2 uvw = face + 0.5;

  float cols = mix(3.0, 5.0, hashT(vTowerSeed));
  vec2 grid = uvw * vec2(cols, cols * 6.0); // taller-than-wide window rows
  vec2 cell = fract(grid);
  vec2 cid = floor(grid);

  // Stable per-window key + its static on/off (unchanged look for most windows).
  float wKey = cid.x * 3.1 + cid.y * 7.7 + vTowerSeed * 19.0;
  float wSeed = hashT(wKey);
  float baseLit = step(0.18, wSeed);

  // Seeded slow flicker: only the ~top ~12% of windows (by flickSeed) animate; the
  // rest stay static. Reversible pure f(uTowerT); rates stay low and the on/off is
  // smoothstepped so it never strobes (well under 3 full-frame flashes/sec, and
  // each animating window is a tiny decorrelated area, never a full-frame flash).
  float flickSeed = hashT(wKey * 1.7 + 4.2);
  float animates = smoothstep(0.86, 0.985, flickSeed);
  float rate = 3.0 + 6.0 * hashT(flickSeed * 3.0); // <= ~9 slow cycles across the film
  float osc = 0.5 + 0.5 * sin(uTowerT * rate + flickSeed * 6.2831853);
  float animLit = smoothstep(0.42, 0.58, osc); // slow on/off for animating windows
  float lit = mix(baseLit, animLit, animates);

  float pane = step(0.14, cell.x) * step(cell.x, 0.86)
             * step(0.16, cell.y) * step(cell.y, 0.84);
  float bright = 0.4 + 0.6 * hashT(cid.x * 1.7 + cid.y * 2.3 + vTowerSeed * 7.0);

  // Slight per-window warm/cool variance around uWinColor (static, cheap): warmer
  // windows gain red / lose blue, cooler ones the reverse.
  float wc = hashT(wKey * 0.7 + 1.3) - 0.5; // -0.5..0.5
  vec3 winTint = uWinColor + vec3(wc * 0.10, wc * 0.02, -wc * 0.14);

  totalEmissiveRadiance += winTint * (side * lit * pane * bright * uWinIntensity);
`;

// Patch a MeshStandardMaterial in place and return the live uniforms. uWinColor /
// uWinIntensity are constant sodium-orange glow; uTowerT is the playhead clock the
// scene mutates each frame (uTowerT.value = playhead.t) to drive the slow flicker.
//
// uWinColor is a saturated sodium-vapor orange (high red, low blue -- a real
// sodium lamp has almost no blue component) rather than a pale amber, and
// uWinIntensity is tuned down from an earlier pass that clipped toward
// washed-out cream under filmic tonemapping at high emissive values; the
// lower-saturation/higher-intensity combination read as "pale cream" windows
// instead of a punchy night-glow -- ADR-0016 art-direction fix.
export function installTowerShader(material: MeshStandardMaterial): TowerUniforms {
  const uniforms: TowerUniforms = {
    uWinColor: { value: new Color(1.0, 0.5, 0.08) },
    uWinIntensity: { value: 1.7 },
    uTowerT: { value: 0 },
  };
  material.onBeforeCompile = (shader) => {
    shader.uniforms.uWinColor = uniforms.uWinColor;
    shader.uniforms.uWinIntensity = uniforms.uWinIntensity;
    shader.uniforms.uTowerT = uniforms.uTowerT;
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", `#include <common>\n${VERTEX_PARS}`)
      .replace("#include <begin_vertex>", `#include <begin_vertex>\n${VERTEX_BODY}`);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", `#include <common>\n${FRAGMENT_PARS}`)
      .replace(
        "#include <emissivemap_fragment>",
        `#include <emissivemap_fragment>\n${FRAGMENT_BODY}`,
      );
  };
  return uniforms;
}
