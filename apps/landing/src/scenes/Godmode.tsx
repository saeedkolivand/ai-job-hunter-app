"use client";

// P4 GODMODE -- beat 5/8, the tonal flip. t-range [0.5, 0.625], waypoint 4->5.
// The camera exits the fried bottom (y=-40) and climbs the fast rise into a warm
// gradient sky, settling at the plateau pose (position (0,20,14), look (0,22,0)),
// so the whole scene is authored in local units around a group parked at world
// (0, 22, 0) -- the look target at the top of the rise. All layout is pure
// authored world position; only the InkStrokes draw-on windows are f(t), so the
// beat scrubs clean both directions (ruling 1: godmode is a draw-while-traveling
// fast leg, so linework reveals as the lens rises and everything has landed by
// the plateau).
//
// Layers, back to front (+z toward camera):
//   - SkyBackdrop : one warm vertical-gradient quad (#ffd884 top -> #ff8a3d
//                   bottom), linear-space shader like Descent's ShaftPaper. One
//                   draw call, the sky the beat is drawn on.
//   - Sun         : a paper sun disc upper-right -- warm-white circle mesh with a
//                   soft radial glow rim (shader, transparent, depthWrite off).
//   - deco        : sun-ray ticks drawing on around the sun, clouds/birds/flower
//                   scattered lean, each on its own staggered draw-on window.
//   - stonks      : the green up-arrow lower-left, a single satisfying sweep.
//   - beat4-dood  : the smug lounging guy, hero-tier InkStrokes, the CROWN drawn
//                   above the quote (ruling 7: look-axis = original read order).
//   - GL text     : the big quote (Patrick Hand), then counter (smallest) + two
//                   body lines below. Original beat4 has no visible headline
//                   beyond an sr-only "godmode", so no Gloria label here.
//
// ponytail: staging is lean -- GL text is static (always drawn while mounted),
// not velocity-gated/screen-space (that art-pass mechanism is deferred). Ceiling:
// no vel gate on this beat's copy yet.

import { useMemo } from "react";
import { Color } from "three";
import { Text } from "@react-three/drei";

import { beat4 } from "@/content/beat4";
import InkStrokes from "@/ink/InkStrokes";
import { PALETTE } from "@/ink/palette";
import { charactersFor, FONT, FONT_TEXTS } from "@/ink/text";

const HAND_CHARS = charactersFor(...FONT_TEXTS.hand);

// Counter line, final values baked in (247 matches, 247 drafts, 6 actually sent
// -- the numbers the semantic layer counts up to). Template pieces imported
// already-escaped from content (the middot separators render in Patrick Hand,
// as they already do in Slump's counter).
const COUNTER =
  beat4.counter.p1 + "247" + beat4.counter.p2 + "247" + beat4.counter.p3 + "6";
// line2 emphasis ("God.") is folded into one plain hand string -- the bold inline
// is polish for the art pass.
const LINE2 = beat4.line2a + beat4.line2b;

// --- shared linear-space vertex shader (same emit rule as Descent's grid) ---
const SKY_VERT = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
}
`;

// Warm sky: vertical gradient, top -> bottom. Colours arrive as linear-decoded
// Color uniforms (THREE.Color parses the sRGB hex), matching the post pipeline.
const SKY_FRAG = /* glsl */ `
uniform vec3 uTop;
uniform vec3 uBottom;
varying vec2 vUv;
void main() {
  gl_FragColor = vec4( mix( uBottom, uTop, vUv.y ), 1.0 );
}
`;

function SkyBackdrop() {
  const uniforms = useMemo(
    () => ({
      uTop: { value: new Color("#ffd884") },
      uBottom: { value: new Color("#ff8a3d") },
    }),
    [],
  );
  return (
    <mesh position={[0, 0, -2]}>
      <planeGeometry args={[44, 30]} />
      <shaderMaterial uniforms={uniforms} vertexShader={SKY_VERT} fragmentShader={SKY_FRAG} />
    </mesh>
  );
}

// Paper sun: warm-white core disc with a soft warm glow rim fading to nothing.
const SUN_FRAG = /* glsl */ `
uniform vec3 uCore;
uniform vec3 uRim;
varying vec2 vUv;
void main() {
  float d = length( vUv - 0.5 ) * 2.0;              // 0 core .. 1 rim
  vec3 col = mix( uCore, uRim, smoothstep( 0.0, 0.85, d ) );
  float disc = 1.0 - smoothstep( 0.52, 0.58, d );   // the paper disc edge
  float glow = ( 1.0 - smoothstep( 0.5, 1.0, d ) ) * 0.45; // soft halo
  gl_FragColor = vec4( col, clamp( disc + glow, 0.0, 1.0 ) );
}
`;

function Sun() {
  const uniforms = useMemo(
    () => ({
      uCore: { value: new Color("#fff4d6") },
      uRim: { value: new Color("#ffb14d") },
    }),
    [],
  );
  return (
    <mesh position={[6.8, 5.2, -1.5]} renderOrder={0}>
      <circleGeometry args={[2.4, 48]} />
      <shaderMaterial
        uniforms={uniforms}
        vertexShader={SKY_VERT}
        fragmentShader={SUN_FRAG}
        transparent
        depthWrite={false}
      />
    </mesh>
  );
}

export default function Godmode() {
  return (
    <group position={[0, 22, 0]}>
      <SkyBackdrop />
      <Sun />

      {/* sun-ray ticks drawing on around the disc */}
      <InkStrokes name="deco-beat4-3" position={[6.8, 5.2, -0.6]} scale={0.055} drawOn={{ t0: 0.53, t1: 0.585 }} />

      {/* clouds, birds, flower -- scattered lean, staggered draw-on */}
      <InkStrokes name="deco-beat4-1" position={[-6.5, 6.8, -0.4]} scale={0.022} drawOn={{ t0: 0.52, t1: 0.55 }} />
      <InkStrokes name="deco-beat4-6" position={[-3.0, 7.2, -0.4]} scale={0.02} drawOn={{ t0: 0.545, t1: 0.575 }} />
      <InkStrokes name="deco-beat4-2" position={[-7.5, 2.2, -0.9]} scale={0.032} drawOn={{ t0: 0.52, t1: 0.56 }} />
      <InkStrokes name="deco-beat4-4" position={[8.0, -1.2, -0.9]} scale={0.03} drawOn={{ t0: 0.55, t1: 0.59 }} />
      <InkStrokes name="deco-beat4-5" position={[-7.6, -4.7, 0.15]} scale={0.03} drawOn={{ t0: 0.57, t1: 0.6 }} />

      {/* stonks up-arrow lower-left: one satisfying single sweep */}
      <InkStrokes name="stonks" position={[-6.4, -2.6, 0.2]} scale={0.026} drawOn={{ t0: 0.55, t1: 0.575 }} />

      {/* the smug lounging guy: hero-tier crown, drawn above the quote */}
      <InkStrokes name="beat4-dood" position={[0, 4.5, 0.3]} scale={0.018} drawOn={{ t0: 0.52, t1: 0.58 }} />

      {/* the big quote (Patrick Hand) */}
      <Text
        font={FONT.hand}
        characters={HAND_CHARS}
        position={[0, 1.4, 0.3]}
        fontSize={0.62}
        maxWidth={10}
        lineHeight={1.15}
        anchorX="center"
        anchorY="middle"
        textAlign="center"
        color={PALETTE.ink}
      >
        {beat4.big}
      </Text>

      {/* counter line: final values, smallest text (crown rule: counters last/smallest) */}
      <Text
        font={FONT.hand}
        characters={HAND_CHARS}
        position={[0, -1.2, 0.3]}
        fontSize={0.3}
        maxWidth={12}
        lineHeight={1.2}
        anchorX="center"
        anchorY="middle"
        textAlign="center"
        color={PALETTE.ink}
        fillOpacity={0.8}
      >
        {COUNTER}
      </Text>

      {/* two body lines below */}
      <Text
        font={FONT.hand}
        characters={HAND_CHARS}
        position={[0, -2.5, 0.3]}
        fontSize={0.38}
        maxWidth={11}
        lineHeight={1.2}
        anchorX="center"
        anchorY="middle"
        textAlign="center"
        color={PALETTE.ink}
      >
        {beat4.line1}
      </Text>
      <Text
        font={FONT.hand}
        characters={HAND_CHARS}
        position={[0, -4.4, 0.3]}
        fontSize={0.38}
        maxWidth={11}
        lineHeight={1.2}
        anchorX="center"
        anchorY="middle"
        textAlign="center"
        color={PALETTE.ink}
      >
        {LINE2}
      </Text>
    </group>
  );
}
