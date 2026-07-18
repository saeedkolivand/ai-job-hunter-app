// The notebook shell materials (cover board + page-stack edge). Both are cheap
// custom ShaderMaterials lit by the SAME hardcoded key direction as the paper +
// ink (matching RipbookExperience's directionalLight) so the whole notebook reads
// under one light without depending on the scene light rig. Output is LINEAR (the
// composer encodes to sRGB), same convention as RipPaperMaterial. Helpers are
// ajh_-prefixed; ASCII only.
//
//   createBoardMaterial()     -- kraft board for the front + back covers. Reuses
//     the shared paper bake (bakes.paperAlbedoRough) sampled at LOW frequency for
//     a coarse board grain, tinted darker toward the kraft-deep board tone.
//     Uniform: uPaper (paper bake, by reference).
//   createStackEdgeMaterial() -- the page-stack block edge: a cheap procedural
//     page-layers stripe + fiber noise in cream. No bake sample.
//
// Both dispose with the mesh that owns them (Book wires the cleanup).

import { ShaderMaterial } from "three";

import { bakes } from "@/bake/bakes";
import { NOISE } from "@/bake/chunks";

const SURFACE_VERT = /* glsl */ `
  varying vec2 vUv;
  varying vec3 vWorldN;
  void main() {
    vUv = uv;
    vWorldN = normalize(mat3(modelMatrix) * normal);
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`;

export function createBoardMaterial(): ShaderMaterial {
  return new ShaderMaterial({
    name: "CoverBoardMaterial",
    uniforms: { uPaper: bakes.paperAlbedoRough },
    vertexShader: SURFACE_VERT,
    fragmentShader: /* glsl */ `
      uniform sampler2D uPaper;
      varying vec2 vUv;
      varying vec3 vWorldN;
      ${NOISE}
      void main() {
        // Coarse board grain: zoom INTO the paper bake (low frequency) and use
        // its luminance as a grain scalar over a dark kraft-board tint.
        float grain = texture2D(uPaper, vUv * 0.35).r;
        vec3 board = ajh_srgb2lin(vec3(0.42, 0.32, 0.20)); // dark kraft board
        vec3 col = board * (0.70 + grain * 0.60);
        vec3 N = normalize(vWorldN);
        if (!gl_FrontFacing) N = -N;
        float lam = clamp(dot(N, normalize(vec3(-0.333, 0.667, 0.667))), 0.0, 1.0);
        col *= 0.50 + 0.50 * lam;
        gl_FragColor = vec4(col, 1.0);
      }
    `,
  });
}

export function createStackEdgeMaterial(): ShaderMaterial {
  return new ShaderMaterial({
    name: "PageStackMaterial",
    vertexShader: SURFACE_VERT,
    fragmentShader: /* glsl */ `
      varying vec2 vUv;
      varying vec3 vWorldN;
      ${NOISE}
      void main() {
        // Layered-paper edge: dense horizontal sheet lines + fiber grain, cream.
        vec3 cream = ajh_srgb2lin(vec3(0.86, 0.80, 0.68));
        vec3 shade = ajh_srgb2lin(vec3(0.62, 0.55, 0.42));
        float lines = smoothstep(0.35, 0.50, fract(vUv.y * 140.0));
        float grain = ajh_vnoise(vUv * vec2(30.0, 220.0));
        vec3 col = mix(shade, cream, lines * 0.6 + grain * 0.4);
        vec3 N = normalize(vWorldN);
        if (!gl_FrontFacing) N = -N;
        float lam = clamp(dot(N, normalize(vec3(-0.333, 0.667, 0.667))), 0.0, 1.0);
        col *= 0.55 + 0.45 * lam;
        gl_FragColor = vec4(col, 1.0);
      }
    `,
  });
}
