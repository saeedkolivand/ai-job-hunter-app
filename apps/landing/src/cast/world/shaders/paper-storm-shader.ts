// Paper-storm vertex/fragment patch for the ONE storm InstancedMesh.
//
// OWNERSHIP: webgl-author wrote the M2 starting point (scene wiring lives in
// PaperStorm.tsx and never reaches into this file). shader-engineer owns the
// flutter model, the recomputed bend normals, and the procedural letter atlas.
//
// CONTRACT: the flutter is a PURE function of the playhead uniform uStormT (never
// a wall-clock uTime), so scrubbing up rewinds the storm exactly and a paused
// scroll freezes it. Per-instance decorrelation rides on the aSeed / aPhase
// InstancedBufferAttributes. One draw call; no CPU per-sheet work -- the sheets
// stream past purely from camera descent + this shader.
//
// The material is a MeshLambertMaterial (cheapest lit material -- per-fragment
// diffuse, no PBR IBL cost) so the fluttering sheets catch the sodium/blue canyon
// light (CanyonWorld's ambient + directional + graded fog). The vertex bend is a
// height field over the sheet plane, and we recompute the object-space normal
// analytically from its derivatives at <beginnormal_vertex> (BEFORE three
// captures vNormal for the per-fragment lambert shading) so the light actually
// tracks the curl instead of the flat unbent plane normal.
//
// The letter impression is applied via the material's `map` slot (installed here
// from the procedural atlas in letter-texture.ts). three handles the uv plumbing
// and sRGB decode; we only offset vMapUv per instance to pick an atlas cell.

import type { MeshLambertMaterial } from "three";

import { getLetterTexture } from "./letter-texture";

export interface StormUniforms {
  uStormT: { value: number };
}

// Injected right after `#include <common>` in the vertex program. Holds the
// per-instance attributes, the playhead clock, the fragment tint varying, and the
// shared analytic flutter helper (ajh_ prefixed to avoid colliding with three's
// injected symbols).
const VERTEX_PARS = /* glsl */ `
  attribute float aSeed;   // [0,1) per-sheet decorrelation
  attribute float aPhase;  // [0,2PI) flutter start phase
  uniform float uStormT;   // playhead t in [0,1] -- the ONLY animation clock
  varying float vBright;   // per-sheet tint variation -> fragment

  // Carries the z displacement from the beginnormal_vertex evaluation (below)
  // through to the begin_vertex inject, so ajh_stormFlutter runs exactly ONCE
  // per vertex instead of once for the normal and again for the displacement.
  float ajh_dz;

  // Analytic sheet flutter: a height field z = f(x, y) over the object-space
  // sheet plane (default normal +Z). Layered so the sheet reads as paper caught
  // in air -- a primary curl across the width plus a secondary phase-offset
  // ripple along the height. PURE f(playhead); no wall-clock. Returns the z
  // displacement and writes the perturbed object-space normal into nrm (from the
  // exact derivatives, so lit shading tracks the curl).
  float ajh_stormFlutter(vec3 p, float seed, float phase, float t, out vec3 nrm) {
    float ph = phase + t * 12.566370614; // 4*PI across the whole film
    const float kx = 3.4;   // primary curl frequency across the width
    const float ky = 6.1;   // secondary ripple frequency along the height
    const float ac = 0.13;  // curl amplitude
    const float ar = 0.045; // ripple amplitude
    float px = p.x * kx + ph;
    float py = p.y * ky + ph * 1.7 + seed * 6.2831853;
    float dz = sin(px) * ac + sin(py) * ar;
    float dzdx = kx * ac * cos(px);
    float dzdy = ky * ar * cos(py);
    nrm = normalize(vec3(-dzdx, -dzdy, 1.0));
    return dz;
  }
`;

// Injected right after `#include <beginnormal_vertex>` (objectNormal exists).
// The ONE evaluation of ajh_stormFlutter for this vertex: replaces the flat
// plane normal with the bend-perturbed one (so per-fragment lambert shading
// catches the canyon light along the curl) and stashes dz in ajh_dz for
// begin_vertex to reuse below -- avoids a second identical evaluation.
const VERTEX_NORMAL = /* glsl */ `
  vec3 ajh_bendNormal;
  ajh_dz = ajh_stormFlutter(position, aSeed, aPhase, uStormT, ajh_bendNormal);
  objectNormal = ajh_bendNormal;
`;

// Injected right after `#include <begin_vertex>` (transformed = position).
// Applies the ajh_dz already computed above, sets the per-sheet tint, and picks
// a per-instance letter-atlas cell by offsetting the map uv (the atlas is 2x2
// -> scale 0.5).
const VERTEX_BODY = /* glsl */ `
  transformed.z += ajh_dz;
  vBright = 0.80 + 0.20 * fract(aSeed * 13.0);
  #ifdef USE_MAP
    float ajh_sel = floor(aSeed * 4.0); // 0..3 atlas cell
    vec2 ajh_cell = vec2(mod(ajh_sel, 2.0), floor(ajh_sel * 0.5));
    vMapUv = vMapUv * 0.5 + ajh_cell * 0.5;
  #endif
`;

const FRAGMENT_PARS = /* glsl */ `
  varying float vBright;
`;

// Injected right after `#include <color_fragment>` (after the letter map has
// already multiplied diffuseColor via <map_fragment>). Per-sheet brightness rides
// an attribute -> NO instanceColor is used, so there is no unset-instance white
// flash path; every instance is valid by construction.
const FRAGMENT_BODY = /* glsl */ `
  diffuseColor.rgb *= vBright;
`;

// Patch a MeshLambertMaterial in place, install the shared procedural letter atlas
// as its map, and return the live uniform object the scene updates each frame
// (uStormT.value = playhead.t). Fail-soft: if a three chunk name ever drifts the
// String.replace is a no-op (no injection, no crash); if the letter atlas cannot
// bake (SSR / no 2D context) the map stays null and the #ifdef USE_MAP path is
// simply skipped.
export function installStormShader(material: MeshLambertMaterial): StormUniforms {
  const uniforms: StormUniforms = { uStormT: { value: 0 } };

  const letter = getLetterTexture();
  if (letter) material.map = letter; // enables USE_MAP -> letter impression + per-instance cell

  material.onBeforeCompile = (shader) => {
    shader.uniforms.uStormT = uniforms.uStormT;
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", `#include <common>\n${VERTEX_PARS}`)
      .replace(
        "#include <beginnormal_vertex>",
        `#include <beginnormal_vertex>\n${VERTEX_NORMAL}`,
      )
      .replace("#include <begin_vertex>", `#include <begin_vertex>\n${VERTEX_BODY}`);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", `#include <common>\n${FRAGMENT_PARS}`)
      .replace("#include <color_fragment>", `#include <color_fragment>\n${FRAGMENT_BODY}`);
  };
  return uniforms;
}
