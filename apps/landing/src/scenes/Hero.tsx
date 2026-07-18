"use client";

// P2 HERO -- the staged desk at the world origin (0,0,0), the hero waypoint's
// look target. Composed for evalPose(0): camera at (0,0,12) looking at origin,
// R3F's default fov 75 -> ~18.4 world units of visible height at the z=0 page.
// All layout lives in world units around the origin so it reads centred at rest.
//
// Layers, back to front (+z toward camera):
//   - GraphPaper : one big quad, a cheap grid ShaderMaterial (paper + faint
//                  1-unit ruling). Cheaper than a fat-line grid (one draw call).
//   - hero-dood  : the hero guy, per-stroke InkStrokes (hero tier) with boil,
//                  standing below the headline. drawOn {0,0} = a zero-width
//                  window, so he is drawn at rest for every t>=0 (the guy is
//                  furniture on load, not a scroll reveal) while still riding the
//                  full-fidelity per-stroke path + line-boil.
//   - dc-arrow + "don't click me" : decor arrow + Caveat sign beside him.
//   - deco-hero-* : scribbles that draw ON early in the hero t-range [0,1/8].
//   - GL text    : h1 (Caveat) split per line, the punchline word "robot" its
//                  own red Text seated with the hero-underline strokes; sub
//                  (Patrick Hand) as one block; kicker (Space Mono, tracked).
//
// ponytail: hero-dood uses the draw path with a zero-width window rather than a
// new "static per-stroke" mode -- ceiling: the fill polys pop in at frame 1 with
// no reveal. Upgrade path: a real at-rest per-stroke branch in InkStrokes.

import { useMemo } from "react";
import { Color, Vector2 } from "three";
import { Text } from "@react-three/drei";

import { hero } from "@/content/hero";
import InkStrokes from "@/ink/InkStrokes";
import { PALETTE } from "@/ink/palette";
import { charactersFor, FONT, FONT_TEXTS } from "@/ink/text";

// --- graph-paper backdrop --------------------------------------------------
// Linear-space output: the scene renders into postprocessing's linear buffer and
// the final EffectPass encodes to sRGB, so a raw ShaderMaterial must emit linear
// (THREE.Color decodes the sRGB hex to linear on upload) -- matches how the
// MeshBasic paper plane read before. fwidth anti-aliases the ruling.
const PAPER_VERT = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
}
`;
const PAPER_FRAG = /* glsl */ `
uniform vec3 uPaper;
uniform vec3 uLine;
uniform vec2 uSize;
varying vec2 vUv;
void main() {
  vec2 c = vUv * uSize;                       // 1 cell == 1 world unit
  vec2 g = abs( fract( c - 0.5 ) - 0.5 ) / fwidth( c );
  float ln = 1.0 - min( min( g.x, g.y ), 1.0 );
  vec3 col = mix( uPaper, uLine, ln * 0.55 );
  gl_FragColor = vec4( col, 1.0 );
}
`;

function GraphPaper() {
  const uniforms = useMemo(
    () => ({
      uPaper: { value: new Color(PALETTE.paper) },
      uLine: { value: new Color(PALETTE.line) },
      uSize: { value: new Vector2(60, 40) },
    }),
    [],
  );
  return (
    <mesh position={[0, 0, -0.8]}>
      <planeGeometry args={[60, 40]} />
      <shaderMaterial uniforms={uniforms} vertexShader={PAPER_VERT} fragmentShader={PAPER_FRAG} />
    </mesh>
  );
}

// --- headline pieces (line 2 lays out around a centred "robot") -------------
// The red "robot" is its own centred Text; "So I built a" (right-anchored) and
// "to do it." (left-anchored) sit on either side. The gap on each side is an
// explicit x-offset -- half of robot's rendered width (~0.98 world at fontSize
// H) plus one space (~0.25) -- so the words land a clean space-gap off the
// punchline. The old 0.95 offset was NARROWER than robot's half-width, so the
// neighbours collided with it and the line read "arobotto"; authoring the gap
// as geometry (rather than trusting a trailing/leading space glyph to survive
// troika's per-segment layout) makes the spacing deterministic.
const L1 = hero.h1a; // "Job hunting broke me."
const P1 = hero.h1b.trim(); // "So I built a"
const ROBOT = hero.h1ul; // "robot"
const P3 = hero.h1c.trim(); // "to do it."
const ROBOT_GAP = 1.23; // robot half-width (~0.98) + one space (~0.25) at H
const KICKER = hero.kicker.toUpperCase();
const SUB = hero.sub + hero.subBold;
const H = 1.05; // headline fontSize (world units)

export default function Hero() {
  return (
    <group position={[0, 0, 0]}>
      <GraphPaper />

      {/* hero guy: per-stroke (hero tier) + boil, drawn at rest */}
      <InkStrokes name="hero-dood" position={[-1, -4.3, 0]} scale={0.028} drawOn={{ t0: 0, t1: 0 }} />

      {/* don't-click arrow + sign beside him */}
      <InkStrokes name="dc-arrow" position={[3.2, -3.2, 0.03]} scale={0.05} drawOn={{ t0: 0.03, t1: 0.06 }} />
      <Text
        font={FONT.caveat}
        characters={charactersFor(...FONT_TEXTS.caveat)}
        position={[3.5, -2.2, 0.1]}
        rotation={[0, 0, -0.14]}
        fontSize={0.42}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.red}
      >
        {hero.dontClick}
      </Text>

      {/* deco scribbles: draw ON early across the hero slice [0, 1/8] */}
      <InkStrokes name="deco-hero-1" position={[-6, 4.2, 0.02]} scale={0.02} drawOn={{ t0: 0.005, t1: 0.03 }} />
      <InkStrokes name="deco-hero-2" position={[6.2, 4, 0.02]} scale={0.025} drawOn={{ t0: 0.01, t1: 0.035 }} />
      <InkStrokes name="deco-hero-3" position={[5.6, 1.6, 0.02]} scale={0.03} drawOn={{ t0: 0.015, t1: 0.04 }} />
      <InkStrokes name="deco-hero-4" position={[-5.2, -2, 0.02]} scale={0.035} drawOn={{ t0: 0.02, t1: 0.045 }} />
      <InkStrokes name="deco-hero-5" position={[5.4, -4.5, 0.02]} scale={0.03} drawOn={{ t0: 0.025, t1: 0.05 }} />

      {/* kicker */}
      <Text
        font={FONT.mono}
        characters={charactersFor(...FONT_TEXTS.mono)}
        position={[0, 5, 0.1]}
        fontSize={0.34}
        letterSpacing={0.12}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
        fillOpacity={0.72}
      >
        {KICKER}
      </Text>

      {/* h1 line 1 */}
      <Text
        font={FONT.caveat}
        characters={charactersFor(...FONT_TEXTS.caveat)}
        position={[0, 3.5, 0.1]}
        fontSize={H}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
        outlineWidth={0.012}
        outlineColor={PALETTE.ink}
      >
        {L1}
      </Text>

      {/* h1 line 2: "So I built a" | robot (red, underlined) | "to do it." */}
      <Text
        font={FONT.caveat}
        characters={charactersFor(...FONT_TEXTS.caveat)}
        position={[-ROBOT_GAP, 2.15, 0.1]}
        fontSize={H}
        anchorX="right"
        anchorY="middle"
        color={PALETTE.ink}
        outlineWidth={0.012}
        outlineColor={PALETTE.ink}
      >
        {P1}
      </Text>
      <Text
        font={FONT.caveat}
        characters={charactersFor(...FONT_TEXTS.caveat)}
        position={[0, 2.15, 0.1]}
        fontSize={H}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.red}
        outlineWidth={0.012}
        outlineColor={PALETTE.red}
      >
        {ROBOT}
      </Text>
      <Text
        font={FONT.caveat}
        characters={charactersFor(...FONT_TEXTS.caveat)}
        position={[ROBOT_GAP, 2.15, 0.1]}
        fontSize={H}
        anchorX="left"
        anchorY="middle"
        color={PALETTE.ink}
        outlineWidth={0.012}
        outlineColor={PALETTE.ink}
      >
        {P3}
      </Text>
      {/* underline strokes seated under "robot" */}
      <InkStrokes name="hero-underline" position={[0, 1.55, 0.05]} scale={0.016} drawOn={{ t0: 0.02, t1: 0.055 }} />

      {/* sub copy: one Patrick Hand block */}
      <Text
        font={FONT.hand}
        characters={charactersFor(...FONT_TEXTS.hand)}
        position={[0, 0.55, 0.1]}
        fontSize={0.4}
        maxWidth={10}
        lineHeight={1.25}
        anchorX="center"
        anchorY="top"
        textAlign="center"
        color={PALETTE.ink}
      >
        {SUB}
      </Text>
    </group>
  );
}
