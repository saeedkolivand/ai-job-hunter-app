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
//
// ANTI-ALIASING (M3 review fix -- webgl-author, localized GLSL change): at
// grazing angles / far distances, the grid's screen-space footprint spans many
// window cells per pixel. The per-cell hashed on/off pattern has no analytic
// pre-filtered form (fwidth() can smooth a single continuous EDGE, but not a hard
// jump between two unrelated per-cell hash values), so sampling it undersampled
// aliased into a glitchy yellow/black barcode/moire. Two mitigations, both keyed
// off fwidth(grid) (the cells-per-pixel footprint): (1) the pane-inset edges
// become an analytic screen-space-sized smoothstep instead of a hard step, and
// (2) the whole per-cell pattern (lit / pane / brightness / tint) fades toward
// its known statistical average once undersampled -- the standard mitigation for
// a hashed pattern that cannot be box-filtered directly, matching what the far
// side of a real window wall does (resolves to a flat average glow, not a
// flickering per-window barcode).
const FRAGMENT_BODY = /* glsl */ `
  float side = step(abs(vObjNormal.y), 0.5);

  // pick the two in-plane axes for the current face, unit box -> [0,1]
  vec2 face = abs(vObjNormal.x) > 0.5
    ? vec2(vObjPos.z, vObjPos.y)
    : vec2(vObjPos.x, vObjPos.y);
  vec2 uvw = face + 0.5;

  // Cell size raised ~2.7x (M3 review round 2 fix): fewer, bigger windows per
  // face -- the style frames show big readable window grids, and the old
  // higher-frequency grid aliased badly at our camera distances even with AA.
  float cols = mix(1.1, 1.8, hashT(vTowerSeed));
  vec2 grid = uvw * vec2(cols, cols * 6.0); // taller-than-wide window rows
  vec2 cell = fract(grid);
  vec2 cid = floor(grid);

  // Cells-per-pixel footprint + the AA blend it drives (0 = crisp near/head-on
  // pattern, 1 = fully averaged far/grazing wash). Hardened (M3 review round 2
  // fix): ramps over [0.2, 0.5] cells/pixel instead of [1, 3] -- moire persisted
  // at grazing angles under the old, looser threshold, so undersampling is now
  // caught much earlier and fully averaged (no residual pattern) by 0.5
  // cells/pixel, well before a pixel starts straddling multiple cells' worth of
  // uncorrelated hash values.
  vec2 gridFw = fwidth(grid);
  float cellsPerPixel = max(gridFw.x, gridFw.y);
  float aaBlend = smoothstep(0.2, 0.5, cellsPerPixel);

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

  // Pane inset edges: analytic screen-space AA (an fwidth-sized smoothstep)
  // instead of a hard step -- the inset boundary softens exactly as fast as the
  // grid itself compresses toward the camera, instead of hard-stepping into a
  // moire barcode at grazing angles.
  vec2 cellFw = fwidth(cell) * 0.5;
  vec2 paneLo = smoothstep(vec2(0.14, 0.16) - cellFw, vec2(0.14, 0.16) + cellFw, cell);
  vec2 paneHi = 1.0 - smoothstep(vec2(0.86, 0.84) - cellFw, vec2(0.86, 0.84) + cellFw, cell);
  float pane = paneLo.x * paneHi.x * paneLo.y * paneHi.y;
  float bright = 0.4 + 0.6 * hashT(cid.x * 1.7 + cid.y * 2.3 + vTowerSeed * 7.0);

  // Slight per-window warm/cool variance around uWinColor (static, cheap): warmer
  // windows gain red / lose blue, cooler ones the reverse.
  float wc = hashT(wKey * 0.7 + 1.3) - 0.5; // -0.5..0.5
  vec3 winTint = uWinColor + vec3(wc * 0.10, wc * 0.02, -wc * 0.14);

  // Fade the whole per-cell pattern toward its known statistical average once
  // undersampled (aaBlend -> 1). AVG_* are the means of the uniform-hash
  // distributions used above (baseLit ~ Bernoulli(0.82), pane coverage from the
  // inset thresholds ~0.72*0.68, bright ~ Uniform[0.4,1.0]) -- so the averaged
  // result reads as a flat, correctly-lit wall glow instead of a barcode.
  const float AVG_LIT = 0.82;
  const float AVG_PANE = 0.49;
  const float AVG_BRIGHT = 0.7;
  float litF = mix(lit, AVG_LIT, aaBlend);
  float paneF = mix(pane, AVG_PANE, aaBlend);
  float brightF = mix(bright, AVG_BRIGHT, aaBlend);
  vec3 tintF = mix(winTint, uWinColor, aaBlend);

  totalEmissiveRadiance += tintF * (side * litF * paneF * brightF * uWinIntensity);
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
