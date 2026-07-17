// Line-boil: a vertex-shader onBeforeCompile patch for the fat-line
// LineMaterial (Line2 / LineSegments2). It jitters each stroke's endpoints by
// 2D hash noise keyed to a STEPPED time uniform so the ink reads as hand-redrawn
// a few times a second instead of wobbling at render framerate.
//
// Why a shader effect and NOT re-setPositions() on the CPU: rewriting the
// interleaved instance buffer every frame would reallocate + re-upload the whole
// geometry of every stroke (an allocation + upload storm). Here the geometry is
// uploaded ONCE; per frame only scalar uniforms change -- uBoilTime (shared, one
// write for the whole page) and the per-material uSeed/uAmp set once at compile.
// uBoilTime is quantised to the tier boilFps via engine/clocks.boilTime, so the
// pattern only re-rolls ~8-10x/second and, being a pure function of that stepped
// value, stays scrub-safe.
//
// Patched vertex-shader locations (three r185 ShaderLib['line'].vertexShader,
// patched by substring so whitespace never matters):
//   1. injected immediately before `void main() {` -- the uBoilTime/uSeed/uAmp
//      uniform declarations plus the ajh_hash2 / ajh_boil helper functions.
//   2. the inline "// camera space" instance-position expressions
//      `vec4( instanceStart, 1.0 )` and `vec4( instanceEnd, 1.0 )` -- the only
//      reads of those two attributes -- are wrapped in ajh_boil(...). The
//      fat-line shader hand-writes this transform inline; there is NO named
//      #include chunk for it, so these two expressions ARE the injection points.
// Displacing the object-space endpoints means every downstream path (the
// WORLD_UNITS tube, the screen-space offset, the USE_DASH distances) follows the
// boiled positions with no further edits.

import type { LineMaterial } from "three/addons/lines/LineMaterial.js";

import { boilTime } from "@/engine/clocks";

// Shared stepped-time uniform: ONE object referenced by every boiled material,
// so a single write per frame advances the boil for the whole page. Updated by
// InkStrokes' priority-0 useFrame through advanceBoil().
export const boilClock: { value: number } = { value: 0 };

export function advanceBoil(elapsed: number, fps: number): void {
  boilClock.value = boilTime(elapsed, fps);
}

// Default displacement amplitude in WORLD units (a subtle paper tremor). Callers
// pass ampLocal = BOIL_AMP / scale so the world-space jitter is scale-invariant:
// the patch displaces object-space coords, which the group's model scale then
// grows back up to BOIL_AMP world units. Tune during hero staging.
export const BOIL_AMP = 0.04;

const BOIL_HEAD = /* glsl */ `
uniform float uBoilTime;
uniform float uSeed;
uniform float uAmp;

vec2 ajh_hash2( vec2 p ) {
  p = vec2( dot( p, vec2( 127.1, 311.7 ) ), dot( p, vec2( 269.5, 183.3 ) ) );
  return fract( sin( p ) * 43758.5453123 ) * 2.0 - 1.0;
}

// Displace an object-space endpoint within the doodle's paper plane (xy); z is
// left untouched so strokes never lift off the page. The position is quantised
// to a coarse cell so a short run of the polyline shares one offset (reads as a
// redrawn segment, not per-vertex fizz); adding the stepped boil time re-rolls
// the whole doodle each boil step, and uSeed decorrelates materials.
vec3 ajh_boil( vec3 p ) {
  vec2 cell = floor( p.xy * 0.15 + uSeed );
  vec2 n = ajh_hash2( cell + uSeed + uBoilTime * 37.0 );
  return vec3( p.xy + n * uAmp, p.z );
}

`;

let seq = 0;

// Patch a LineMaterial in place with the boil displacement. `ampLocal` is in the
// material's object-space units (pass BOIL_AMP / group-scale for a world-unit
// amplitude). Returns the same material for chaining.
export function patchBoil(mat: LineMaterial, ampLocal: number): LineMaterial {
  const uSeed = { value: (seq++ * 0.6180339887) % 10.0 };
  const uAmp = { value: ampLocal };

  mat.onBeforeCompile = (shader) => {
    shader.uniforms.uBoilTime = boilClock;
    shader.uniforms.uSeed = uSeed;
    shader.uniforms.uAmp = uAmp;
    shader.vertexShader = shader.vertexShader
      .replace("void main() {", BOIL_HEAD + "void main() {")
      .replace("vec4( instanceStart, 1.0 )", "vec4( ajh_boil( instanceStart ), 1.0 )")
      .replace("vec4( instanceEnd, 1.0 )", "vec4( ajh_boil( instanceEnd ), 1.0 )");
  };
  // Keep boiled programs from sharing a compiled shader with any unpatched
  // LineMaterial of identical parameters -- their GLSL differs. Boiled materials
  // that differ by define (dashed / vertexColors) still split on the base key.
  mat.customProgramCacheKey = () => "ink-boil";

  return mat;
}
