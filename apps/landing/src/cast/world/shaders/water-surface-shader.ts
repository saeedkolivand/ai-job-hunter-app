// Gerstner water-surface vertex/fragment patch for the bounded ocean patch (M3).
//
// OWNERSHIP: webgl-author seeded the onBeforeCompile scaffold + the analytic
// Gerstner displacement + normal; shader-engineer refined the fragment look:
// (1) a real derivative-based multi-octave micro-detail normal map (replacing the
// 2-sine placeholder) and (2) a fresnel-graded horizon sky reflection. The
// Gerstner VERTEX displacement is UNCHANGED -- it must stay bit-identical with
// water-layout.gerstnerSurface (shared GERSTNER_WAVES). The cursor ripple-drag
// DataTexture wake + differential-area caustics stay TODO (they need a scene-fed
// wake texture, i.e. scene wiring, so they are a separate pass). Scene wiring
// (WaterSurface.tsx) never reaches into this file.
//
// CONTRACT: the displacement is a PURE function of the playhead uniform uWaterT
// (never a wall-clock uTime), so scrubbing up rewinds the swell exactly and a
// paused scroll freezes it. The wave set is generated from the SINGLE source of
// truth in water-layout.ts (GERSTNER_WAVES), so the surface this shader draws
// matches the surface the CPU floating-letter layer rides (gerstnerSurface).

import type { MeshStandardMaterial } from "three";

import { GERSTNER_WAVES } from "../water-layout";

export interface WaterUniforms {
  uWaterT: { value: number };
}

// GLSL float literal (always has a decimal point so it is a float, not an int).
function f(n: number): string {
  return n.toFixed(6);
}

// Build the GLSL const wave arrays from the CPU wave list (directions
// pre-normalized here so the shader loop does not have to). vec4 = (dirX, dirZ,
// amp, wavelength); vec2 = (steepness, omega).
function waveArraysGLSL(): string {
  const wv: string[] = [];
  const wq: string[] = [];
  for (const w of GERSTNER_WAVES) {
    const dl = Math.hypot(w.dx, w.dz) || 1;
    wv.push(`vec4(${f(w.dx / dl)}, ${f(w.dz / dl)}, ${f(w.amp)}, ${f(w.len)})`);
    wq.push(`vec2(${f(w.steep)}, ${f(w.omega)})`);
  }
  const n = GERSTNER_WAVES.length;
  return `
  const int AJH_NW = ${n};
  const vec4 AJH_WV[${n}] = vec4[](${wv.join(", ")});
  const vec2 AJH_WQ[${n}] = vec2[](${wq.join(", ")});`;
}

// Head of the vertex pars; the generated wave arrays are spliced between this and
// the gerstner function so the const arrays are declared before the loop reads them.
const VERTEX_PARS_HEAD = /* glsl */ `
  uniform float uWaterT;   // playhead t in [0,1] -- the ONLY animation clock
  varying vec2 vAjhXZ;     // rest xz -> fragment (procedural micro-detail domain)
  varying vec3 vAjhView;   // view-space displaced position -> fragment (view dir + tangents)
  vec3 ajh_pos;            // displaced object position, shared normal->body`;

const VERTEX_PARS_FN = /* glsl */ `
  // Sum-of-Gerstner-waves displacement + analytic normal. Pure f(xz, t); mirrors
  // water-layout.gerstnerSurface exactly (same constants, same math).
  vec3 ajh_gerstner(vec2 xz, float t, out vec3 nrm) {
    vec3 p = vec3(xz.x, 0.0, xz.y);
    float sNx = 0.0; float sNy = 0.0; float sNz = 0.0;
    for (int i = 0; i < AJH_NW; i++) {
      vec4 wv = AJH_WV[i];
      vec2 wq = AJH_WQ[i];
      vec2 dir = wv.xy;
      float amp = wv.z;
      float k = 6.283185307 / wv.w;
      float steep = wq.x;
      float phase = k * dot(dir, xz) + t * wq.y;
      float c = cos(phase); float s = sin(phase);
      p.x += steep * amp * dir.x * c;
      p.z += steep * amp * dir.y * c;
      p.y += amp * s;
      float wa = k * amp;
      sNx += dir.x * wa * c;
      sNz += dir.y * wa * c;
      sNy += steep * wa * s;
    }
    nrm = normalize(vec3(-sNx, 1.0 - sNy, -sNz));
    return p;
  }
`;

// The ONE Gerstner evaluation per vertex: perturb the normal (so lit shading +
// specular track the swell) and stash the displaced position for begin_vertex.
const VERTEX_NORMAL = /* glsl */ `
  vec3 ajh_n;
  ajh_pos = ajh_gerstner(position.xz, uWaterT, ajh_n);
  objectNormal = ajh_n;
`;

const VERTEX_BODY = /* glsl */ `
  transformed = ajh_pos;
  vAjhXZ = position.xz;
  // View-space displaced position: self-contained source for the fragment view
  // direction + tangent frame (never depends on three's internal vViewPosition).
  vAjhView = (modelViewMatrix * vec4(ajh_pos, 1.0)).xyz;
`;

const FRAGMENT_PARS = /* glsl */ `
  uniform float uWaterT;
  varying vec2 vAjhXZ;      // rest xz -> procedural micro-detail domain
  varying vec3 vAjhView;    // view-space surface position (self-contained view dir + tangents)

  // Fresnel term computed once the shading normal is final (normal_fragment_maps)
  // and reused for the horizon reflection at lights_fragment_end. Declared global
  // (like the storm shader's ajh_dz) so a theoretical chunk-name drift on one
  // inject can never leave it undeclared.
  float ajh_fres;

  // Cool moonlit sky tint the water reflects at grazing angles, plus the micro-
  // detail tilt strength. Both kept modest so the floating-letter layer stays
  // readable: looking straight DOWN onto the letters the fresnel ~ 0, so the
  // water stays dark under them; only the far horizon brightens, where few
  // letters sit, and the micro-tilt is a gentle sparkle, not a wash.
  const vec3 AJH_SKY_REFLECT = vec3(0.05, 0.09, 0.15);
  const float AJH_MICRO = 0.025;

  // Analytic gradient of a 3-octave travelling ripple height field h(xz, t):
  // d/dp of sum( amp * sin(dot(p,dir)*freq + t*speed) ) = sum( dir*freq*amp*cos ).
  // Pure f(t, xz) -- the micro-glints animate with the swell and rewind exactly.
  vec2 ajh_microGrad(vec2 p, float t) {
    vec2 g = vec2(0.0);
    vec2 d1 = vec2(0.94, 0.34); float f1 = 1.6;
    g += d1 * f1 * (0.90 * cos(dot(p, d1) * f1 + t * 26.0));
    vec2 d2 = vec2(-0.42, 0.91); float f2 = 3.3;
    g += d2 * f2 * (0.50 * cos(dot(p, d2) * f2 + t * 37.0));
    vec2 d3 = vec2(0.71, -0.70); float f3 = 6.7;
    g += d3 * f3 * (0.26 * cos(dot(p, d3) * f3 - t * 51.0));
    return g;
  }
`;

// Derivative-based multi-octave micro-detail normal map (replaces the 2-sine
// placeholder). Builds a view-space tangent frame from the surface's screen-space
// derivatives and tilts the shading normal by the analytic ripple gradient, so
// the low-roughness moonlit specular sparkles with high-frequency detail the
// coarse Gerstner vertex mesh cannot carry. Then measures the fresnel toward the
// horizon (camera at the view-space origin) for the sky reflection below.
const FRAGMENT_NORMAL = /* glsl */ `
  vec2 ajh_g = ajh_microGrad(vAjhXZ, uWaterT);
  vec3 ajh_dpx = dFdx(vAjhView);
  vec3 ajh_T = dot(ajh_dpx, ajh_dpx) > 1e-10 ? normalize(ajh_dpx) : vec3(1.0, 0.0, 0.0);
  vec3 ajh_B = normalize(cross(normal, ajh_T));
  normal = normalize(normal - (ajh_T * ajh_g.x + ajh_B * ajh_g.y) * AJH_MICRO);

  vec3 ajh_V = normalize(-vAjhView);
  ajh_fres = pow(1.0 - clamp(dot(normal, ajh_V), 0.0, 1.0), 5.0);
`;

// Fresnel-graded reflectivity toward the horizon: add a cool sky tint to the
// indirect specular, scaled by the fresnel term. Grazing angles (the far
// horizon) reflect the sky; head-on (looking down onto the letters) adds nothing.
const FRAGMENT_REFLECT = /* glsl */ `
  reflectedLight.indirectSpecular += ajh_fres * AJH_SKY_REFLECT;
`;

// Patch a MeshStandardMaterial in place and return the live uniform object the
// scene updates each frame (uWaterT.value = playhead.t). Fail-soft: if a three
// chunk name ever drifts the String.replace is a no-op (no injection, no crash).
export function installWaterShader(material: MeshStandardMaterial): WaterUniforms {
  const uniforms: WaterUniforms = { uWaterT: { value: 0 } };
  const pars = `${VERTEX_PARS_HEAD}\n${waveArraysGLSL()}\n${VERTEX_PARS_FN}`;

  material.onBeforeCompile = (shader) => {
    shader.uniforms.uWaterT = uniforms.uWaterT;
    shader.vertexShader = shader.vertexShader
      .replace("#include <common>", `#include <common>\n${pars}`)
      .replace("#include <beginnormal_vertex>", `#include <beginnormal_vertex>\n${VERTEX_NORMAL}`)
      .replace("#include <begin_vertex>", `#include <begin_vertex>\n${VERTEX_BODY}`);
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", `#include <common>\n${FRAGMENT_PARS}`)
      .replace(
        "#include <normal_fragment_maps>",
        `#include <normal_fragment_maps>\n${FRAGMENT_NORMAL}`,
      )
      .replace(
        "#include <lights_fragment_end>",
        `#include <lights_fragment_end>\n${FRAGMENT_REFLECT}`,
      );
  };
  return uniforms;
}
