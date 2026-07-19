// Additive god-ray shaft patch (M3 fallback path -- see the godrays decision in
// the handoff / webgl-standards). A handful of these cone meshes stand in for
// volumetric god-rays WITHOUT a post composer (which M3 does not build). The real
// three-good-godrays GodraysPass is a middle composer pass and belongs to the
// post-chain milestone; this is the cheap, always-available LOW/failure path.
//
// OWNERSHIP: webgl-author seeded the soft length + facing fade; shader-engineer
// refined the look: fresnel-style edge softening (head-on core, soft grazing
// silhouette), along-shaft depth-graded thinning, and a subtle animated density
// band. Scene wiring (DeepScene.tsx) never reaches into this file.
//
// CONTRACT: appearance is a PURE function of the uStrength uniform, itself a pure
// f(t) (water-layout.godrayStrength) -- so the shafts thin band by band into the
// deep and rewind exactly. The density band is phased by uStrength too, so it
// stays pure f(t) with NO wall-clock and NO extra per-frame uniform (the scene
// only ever sets uStrength; adding a monotonic deep-time uniform would be a
// scene-wiring change owned by webgl-author, deliberately avoided here).

import type { MeshBasicMaterial } from "three";

export interface GodrayUniforms {
  uStrength: { value: number };
  uShaftColor: { value: unknown };
}

// Own varyings + our own length coord derived from object-space position rather
// than the geometry's `uv` attribute. NOTE: three DOES declare `uv` as a vertex
// attribute unconditionally whenever the bound geometry provides one (ConeGeometry
// does) -- WebGLProgram emits the attribute straight from the geometry's own
// attributes, independent of the `USE_UV` macro (that macro only gates the `vUv`
// VARYING assignment inside three's own chunks). We still avoid it here on
// purpose: deriving vAjhV from the unit cone's own object-space y keeps this
// shader correct even if the geometry is ever swapped for one with a different
// (or no) uv layout, with no extra attribute lookup. vAjhV = cone length coord
// (base -0.5 -> apex +0.5, remapped to 0..1); vAjhFacing = how head-on the
// surface faces the camera (softens the silhouette edge).
const VERTEX_PARS = /* glsl */ `
  varying float vAjhV;
  varying float vAjhFacing;
`;

const VERTEX_BODY = /* glsl */ `
  vAjhV = position.y + 0.5;
  vec3 ajh_n = normalize(normalMatrix * normal);
  vec3 ajh_view = normalize(-mvPosition.xyz);
  vAjhFacing = abs(dot(ajh_n, ajh_view));
`;

const FRAGMENT_PARS = /* glsl */ `
  varying float vAjhV;
  varying float vAjhFacing;
  uniform float uStrength;
  uniform vec3 uShaftColor;
`;

// Soft shaft body -> additive output alpha. Layers four falloffs:
//   ajh_len   -- along-shaft length window (fade in from the deep base, taper
//                before the surface apex) so the cone has no hard caps.
//   ajh_depth -- depth-graded thinning: full near the surface apex (vAjhV -> 1),
//                fainter into the deep base (vAjhV -> 0). The band-by-band thinning
//                with the playhead itself is carried by uStrength (godrayStrength).
//   ajh_core  -- fresnel-style edge softening: a head-on core (facing -> 1) with a
//                soft grazing silhouette (facing -> 0), so the cone never shows a
//                hard geometric outline against the deep.
//   ajh_dens  -- a subtle density band travelling along the shaft, phased by
//                uStrength so it drifts as the rays thin and rewinds exactly.
const FRAGMENT_BODY = /* glsl */ `
  float ajh_len = smoothstep(0.0, 0.22, vAjhV) * (1.0 - smoothstep(0.6, 1.0, vAjhV));
  float ajh_depth = mix(0.4, 1.0, vAjhV);
  float ajh_fres = pow(clamp(vAjhFacing, 0.0, 1.0), 1.6);
  float ajh_edge = smoothstep(0.0, 0.26, vAjhFacing);
  float ajh_core = mix(0.28, 1.0, ajh_fres) * ajh_edge;
  float ajh_dens = 0.85 + 0.15 * sin(vAjhV * 18.0 - uStrength * 12.0);
  diffuseColor.rgb = uShaftColor;
  diffuseColor.a *= uStrength * ajh_len * ajh_depth * ajh_core * ajh_dens;
`;

// Patch a transparent + additive MeshBasicMaterial. The caller sets
// uStrength.value = godrayStrength(t) each frame. Fail-soft on chunk drift.
export function installGodrayShaftShader(
  material: MeshBasicMaterial,
  shaftColor: unknown,
): GodrayUniforms {
  const uniforms: GodrayUniforms = {
    uStrength: { value: 0 },
    uShaftColor: { value: shaftColor },
  };

  material.onBeforeCompile = (shader) => {
    shader.uniforms.uStrength = uniforms.uStrength;
    shader.uniforms.uShaftColor = uniforms.uShaftColor;
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", `#include <common>\n${VERTEX_PARS}`)
      .replace("#include <project_vertex>", `#include <project_vertex>\n${VERTEX_BODY}`);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", `#include <common>\n${FRAGMENT_PARS}`)
      .replace("#include <color_fragment>", `#include <color_fragment>\n${FRAGMENT_BODY}`);
  };
  return uniforms;
}
