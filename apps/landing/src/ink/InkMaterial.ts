// The RIPBOOK ink system, proven on the desk ink dummy (desk/InkDummy). Two
// material slots, both keyed off the shared singleton uniforms BY REFERENCE
// (uBoil from engine/uniforms drives the hand-drawn boil step; uResolution keeps
// the OUTLINE screen-constant; uHatch is the baked hatch atlas holder):
//
//   createInkMaterial()        -- 3-band toon fill. Two smoothstep thresholds on
//     a full-lambert N.L (thresholds sit inside the visible N.L range so all three
//     bands read at the fixed top-down framing) split the surface into band 0 =
//     bare kraft paper, band 1 = single-direction pencil hatch, band 2 =
//     cross-hatch (single+cross, visibly denser). The hatch is sampled TRI-PLANAR
//     in object space (no UVs needed on the merged dummy), distance-compensated to
//     a screen-stable stroke size (UV scaled by a power-of-two of camera distance
//     so strokes never swim -- no uResolution needed here), phase-jittered by the
//     stepped uBoil, with the atlas blue-noise channel (A) multiplied INSIDE
//     stroke coverage only for a pencil grain.
//
//   createInkOutlineMaterial() -- inverted-hull outline. The vertex shader
//     extrudes along the normal in CLIP space to a screen-constant width
//     (uWidthPx px, via clip.w / uResolution), with a boil-stepped curl wobble
//     and a per-vertex pressure width jitter (hash of position + uBoil). Front
//     faces are culled (BackSide) so only the rim shows behind the fill; flat ink
//     colour with a slight opacity jitter. Because the width is shader-driven the
//     dummy no longer needs to scale the hull (OUTLINE_SCALE is back to 1).
//
// Everything is pure f(uniforms): all jitter re-seeds from the stepped uBoil, so
// nothing accumulates time state and the whole prop boils on one step. Ink colour
// is #17130F on kraft. Helpers are ajh_-prefixed.

import { BackSide, ShaderMaterial } from "three";

import { bakes } from "@/bake/bakes";
import { NOISE } from "@/bake/chunks";
import { uBoil, uResolution } from "@/engine/uniforms";

// Outline width in device pixels (webgl-standards: 1.5-3px screen-constant).
const OUTLINE_WIDTH_PX = 2.2;

export function createInkMaterial(): ShaderMaterial {
  return new ShaderMaterial({
    name: "InkMaterial",
    uniforms: {
      uBoil,
      uHatch: bakes.hatch,
    },
    vertexShader: /* glsl */ `
      varying vec3 vObjPos;
      varying vec3 vObjN;
      varying vec3 vWorldPos;
      varying vec3 vWorldN;
      void main() {
        vObjPos = position;
        vObjN = normal;
        vec4 wp = modelMatrix * vec4(position, 1.0);
        vWorldPos = wp.xyz;
        vWorldN = normalize(mat3(modelMatrix) * normal);
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      uniform float uBoil;
      uniform sampler2D uHatch;

      varying vec3 vObjPos;
      varying vec3 vObjN;
      varying vec3 vWorldPos;
      varying vec3 vWorldN;

      ${NOISE}

      const vec3 AJH_LIGHT = vec3(-0.333, 0.667, 0.667);
      const float AJH_HSCALE = 5.0; // strokes-per-world unit at lod 1

      // Tri-planar hatch lookup. p/n are object-space; comp is the power-of-two
      // camera-distance step. UV divided by comp -> screen-stable stroke size.
      // fract() wraps the atlas (RepeatWrapping on the RT would drop the fract,
      // but fract is safe either way for this small prop).
      vec4 ajh_triHatch(vec3 p, vec3 n, float comp) {
        vec3 an = abs(normalize(n));
        an /= (an.x + an.y + an.z);
        vec2 j = (vec2(ajh_hash11(uBoil * 13.1), ajh_hash11(uBoil * 7.3 + 2.0)) - 0.5) * 0.18;
        float s = AJH_HSCALE / comp;
        vec4 hx = texture2D(uHatch, fract(p.zy * s + j));
        vec4 hy = texture2D(uHatch, fract(p.xz * s + j));
        vec4 hz = texture2D(uHatch, fract(p.xy * s + j));
        return hx * an.x + hy * an.y + hz * an.z;
      }

      void main() {
        vec3 N = normalize(vWorldN);
        vec3 L = normalize(AJH_LIGHT);
        // Full lambert: the shadowed hemisphere reaches N.L = 0, so band 2 is
        // reachable at the fixed top-down framing. (Half-lambert compressed the
        // dark end above the darkest visible pixel, hiding the cross-hatch band.)
        float ndl = clamp(dot(N, L), 0.0, 1.0);

        float dist = length(cameraPosition - vWorldPos);
        float comp = exp2(floor(log2(max(dist, 0.001))));
        vec4 h = ajh_triHatch(vObjPos, vObjN, comp);
        float blue = h.a;

        // Pencil grain: blue-noise multiplied INSIDE stroke coverage only.
        float single = h.r * mix(0.55, 1.0, blue);
        float crossH = h.g * mix(0.55, 1.0, blue);

        // Two smoothstep thresholds -> 3 bands, thresholds inside the visible
        // N.L range so all three read: band0 bright = bare paper, band1 mid =
        // single hatch, band2 dark = cross-hatch. band2 adds single+cross so it
        // is visibly DENSER than band1 (not just the same coverage darker).
        const float EW = 0.06;
        float b1 = smoothstep(0.26 - EW, 0.26 + EW, ndl);
        float b2 = smoothstep(0.56 - EW, 0.56 + EW, ndl);
        float wDark = 1.0 - b1;
        float wMid = b1 * (1.0 - b2);
        float band1Ink = single;
        float band2Ink = clamp(single + crossH, 0.0, 1.0);
        float ink = clamp(wMid * band1Ink + wDark * band2Ink, 0.0, 1.0);

        vec3 kraft = ajh_srgb2lin(vec3(0.788, 0.659, 0.463)); // #C9A876
        vec3 INK = ajh_srgb2lin(vec3(0.090, 0.075, 0.059));   // #17130F
        vec3 col = mix(kraft, INK, ink);
        gl_FragColor = vec4(col, 1.0);
      }
    `,
  });
}

export function createInkOutlineMaterial(): ShaderMaterial {
  return new ShaderMaterial({
    name: "InkOutline",
    side: BackSide, // cull front faces -> only the rim shows behind the fill
    transparent: true, // slight opacity jitter reads as pen pressure
    uniforms: {
      uBoil,
      uResolution,
      uWidthPx: { value: OUTLINE_WIDTH_PX },
    },
    vertexShader: /* glsl */ `
      uniform float uBoil;
      uniform vec2 uResolution;
      uniform float uWidthPx;

      ${NOISE}

      void main() {
        // Boil-stepped curl wobble along the normal + a tiny tangential jitter,
        // re-seeded off the stepped uBoil (never per-frame smooth -> hand-drawn).
        float wob = ajh_vnoise(position.xy * 7.0 + position.z * 3.0 + uBoil * 3.3) - 0.5;
        vec2 tj = vec2(
          ajh_vnoise(position.yz * 6.0 + uBoil * 2.1),
          ajh_vnoise(position.zx * 6.0 + uBoil * 4.7)
        ) - 0.5;
        vec3 pos = position + normal * wob * 0.012 + vec3(tj * 0.006, 0.0);

        // Per-vertex pressure width jitter (hash of position + boil).
        float pj = 0.7 + 0.6 * ajh_hash11(dot(position, vec3(12.3, 7.7, 3.1)) + uBoil * 5.0);

        vec4 clip = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
        vec3 vN = normalize(normalMatrix * normal);
        // Guard the clip-space normal: a hull normal pointing down the view axis
        // projects to ~(0,0) in clip xy, and normalize() of that is NaN. Fall
        // back to +y so those verts extrude a stable (if arbitrary) direction.
        vec2 cxy = (projectionMatrix * vec4(vN, 0.0)).xy;
        float clen = length(cxy);
        vec2 cN = clen > 1e-4 ? cxy / clen : vec2(0.0, 1.0);
        clip.xy += cN * uWidthPx * pj * 2.0 / uResolution * clip.w;
        gl_Position = clip;
      }
    `,
    fragmentShader: /* glsl */ `
      uniform float uBoil;
      ${NOISE}
      void main() {
        vec3 INK = ajh_srgb2lin(vec3(0.055, 0.050, 0.045));
        float op = 0.9 + 0.1 * ajh_hash11(uBoil * 3.1);
        gl_FragColor = vec4(INK, op);
      }
    `,
  });
}
