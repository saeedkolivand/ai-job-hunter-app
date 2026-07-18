// The desk surface material: a dark, low-contrast wood/felt (#3E3226) with subtle
// two-scale variation. It stays QUIET on purpose -- the desk sits behind the
// notebook and gets tilt-shifted out of focus later, so the material trades any
// detail for calm. Lit by the same hardcoded key direction as the rest of the
// notebook; output LINEAR (composer encodes to sRGB). Helpers ajh_-prefixed.

import { ShaderMaterial } from "three";

import { NOISE } from "@/bake/chunks";

export function createDeskMaterial(): ShaderMaterial {
  return new ShaderMaterial({
    name: "DeskMaterial",
    vertexShader: /* glsl */ `
      varying vec2 vUv;
      varying vec3 vWorldN;
      void main() {
        vUv = uv;
        vWorldN = normalize(mat3(modelMatrix) * normal);
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      varying vec2 vUv;
      varying vec3 vWorldN;
      ${NOISE}
      void main() {
        vec3 felt = ajh_srgb2lin(vec3(0.243, 0.196, 0.149)); // #3E3226
        float n = ajh_fbm5(vUv * 40.0) - 0.5;               // coarse mottle
        float n2 = ajh_vnoise(vUv * 380.0) - 0.5;           // fine felt speckle
        vec3 col = felt * (1.0 + n * 0.10 + n2 * 0.06);
        vec3 N = normalize(vWorldN);
        float lam = clamp(dot(N, normalize(vec3(-0.333, 0.667, 0.667))), 0.0, 1.0);
        col *= 0.70 + 0.30 * lam; // low contrast, quiet
        gl_FragColor = vec4(col, 1.0);
      }
    `,
  });
}
