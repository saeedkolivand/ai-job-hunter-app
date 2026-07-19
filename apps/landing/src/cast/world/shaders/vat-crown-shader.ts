// In-house VAT decode vertex patch for the splash crown (M3).
//
// OWNERSHIP: webgl-author seeded the onBeforeCompile scaffold + the position-
// texture sampling that plays the baked crown (index/interp math from the
// unit-tested engine/vat.ts). shader-engineer refined the GLSL so the crown
// LIGHTS correctly and reads as foam:
//   - Normal reconstruction: the VAT decode moves the vertices but not the flat
//     rest-disc normal, so the fragment rebuilds a face normal from the decoded
//     view-space position via its screen derivatives (needs no baked normal
//     channel and is a drop-in for the real Houdini bake, which ships a normal
//     texture in the SAME row=frame/column=vertex layout -- decode it here the
//     same way the position is decoded, remap [0,1]->[-1,1], and set objectNormal).
//   - Foam brightening at the crest, keyed off the decoded displacement height.
// The frame-pair uniforms (uRowA/uRowB/uBlend) are unchanged.
//
// CONTRACT: playback is pure f(t). The CPU (SplashCrown.tsx) computes the
// deterministic frame pair + blend from scene-2 progress via vatFrameIndex and
// feeds them as uniforms; this shader only samples row A and row B for the vertex
// and lerps -- so scrubbing the splash is just reading a texture at time t,
// reversible and cheap. Each ROW is a frame, each COLUMN a baked vertex (indexed
// by the aVatId attribute the geometry carries in bake order).

import type { Material } from "three";

export interface VatCrownUniforms {
  uVatTex: { value: unknown };
  uVatWidth: { value: number };
  uRowA: { value: number };
  uRowB: { value: number };
  uBlend: { value: number };
}

const VERTEX_PARS = /* glsl */ `
  attribute float aVatId;    // baked vertex index (texture column)
  uniform sampler2D uVatTex; // baked positions: row = frame, column = vertex
  uniform float uVatWidth;   // baked vertex count (texture width)
  uniform float uRowA;       // V of the lower frame row
  uniform float uRowB;       // V of the upper frame row
  uniform float uBlend;      // lerp weight A -> B, in [0,1]
  varying vec3 vAjhView;     // view-space decoded position -> fragment normal reconstruction
  varying float vAjhFoam;    // decoded crest height -> foam brightening
`;

// Replace the geometry position with the interpolated baked crown position. Two
// nearest-filtered samples (one per frame row) lerped by the CPU-computed blend
// -- the interpolation weight is deterministic, so playback is pure f(t). Also
// carry the view-space position (for the fragment face-normal reconstruction) and
// the crest height (for foam) forward as varyings.
const VERTEX_BODY = /* glsl */ `
  float ajh_u = (aVatId + 0.5) / uVatWidth;
  vec3 ajh_pA = texture2D(uVatTex, vec2(ajh_u, uRowA)).xyz;
  vec3 ajh_pB = texture2D(uVatTex, vec2(ajh_u, uRowB)).xyz;
  transformed = mix(ajh_pA, ajh_pB, uBlend);
  vAjhView = (modelViewMatrix * vec4(transformed, 1.0)).xyz;
  vAjhFoam = transformed.y; // crown height above the flat rest disc = crest magnitude
`;

const FRAGMENT_PARS = /* glsl */ `
  varying vec3 vAjhView;
  varying float vAjhFoam;
`;

// Reconstruct the shading normal from the decoded geometry. The VAT decode
// displaces the vertices but leaves three's rest-disc normal, so the crown would
// light as a flat plane. Rebuild a per-fragment face normal from the screen-space
// derivatives of the decoded view-space position and orient it toward the camera
// (view-space origin) so the DOUBLE-sided crown lights on both faces.
const FRAGMENT_NORMAL = /* glsl */ `
  vec3 ajh_fn = normalize(cross(dFdx(vAjhView), dFdy(vAjhView)));
  vec3 ajh_V = normalize(-vAjhView);
  if (dot(ajh_fn, ajh_V) < 0.0) ajh_fn = -ajh_fn;
  normal = ajh_fn;
`;

// Foam-ish brightening at the crest: the higher the decoded displacement, the
// whiter/foamier the paper-ocean spray. Pure f(t) via the decoded position, so it
// erupts and recedes with the splash and rewinds exactly.
const FRAGMENT_FOAM = /* glsl */ `
  float ajh_foam = smoothstep(0.6, 3.2, vAjhFoam);
  diffuseColor.rgb = mix(diffuseColor.rgb, vec3(0.96, 0.99, 1.0), ajh_foam * 0.7);
`;

// Patch any position-based material (MeshStandardMaterial here) to play the VAT.
// The caller owns the uniform object and updates uRowA / uRowB / uBlend each frame
// from vatFrameIndex + frameRowV. Fail-soft: a drifted chunk name is a no-op.
export function installVatCrownShader(
  material: Material,
  vatTexture: unknown,
  vertices: number,
): VatCrownUniforms {
  const uniforms: VatCrownUniforms = {
    uVatTex: { value: vatTexture },
    uVatWidth: { value: vertices },
    uRowA: { value: 0 },
    uRowB: { value: 0 },
    uBlend: { value: 0 },
  };

  material.onBeforeCompile = (shader) => {
    shader.uniforms.uVatTex = uniforms.uVatTex;
    shader.uniforms.uVatWidth = uniforms.uVatWidth;
    shader.uniforms.uRowA = uniforms.uRowA;
    shader.uniforms.uRowB = uniforms.uRowB;
    shader.uniforms.uBlend = uniforms.uBlend;
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", `#include <common>\n${VERTEX_PARS}`)
      .replace("#include <begin_vertex>", `#include <begin_vertex>\n${VERTEX_BODY}`);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", `#include <common>\n${FRAGMENT_PARS}`)
      .replace(
        "#include <normal_fragment_maps>",
        `#include <normal_fragment_maps>\n${FRAGMENT_NORMAL}`,
      )
      .replace("#include <color_fragment>", `#include <color_fragment>\n${FRAGMENT_FOAM}`);
  };
  return uniforms;
}
