"use client";

// P5 FINALE -- beat 8/8, t-range [0.875, 1.0], waypoint 7 -> 8. The camera sweeps
// off the testimonials wall and settles back at the desk (world origin (0,0,0),
// the finale waypoint's look target) -- a SECOND desk vignette that echoes the
// hero: the same graph-paper ground, a hand at the desk, the honest close, and
// the one button the whole page has been walking toward. Composed for evalPose(1)
// (camera (0,0,12) -> origin, R3F fov 75 -> ~18.4 world units of visible height),
// so the column reads centred at rest -- the same framing the hero opened on
// (hero and finale never co-mount: beats 0 and 7 are never neighbours).
//
// Layout is a vertical column (crown rule: doodle-as-crown on top, then original
// read order down): finale-dood -> wavy divider -> honest paragraph -> CTA button
// -> source / built-with / byline -> the closing heart. Only the ink (dood,
// inklines) reveals as f(t); the GL text is static while mounted -- lean staging,
// the art pass moves body copy to a DOM overlay later (ruling: GL text = display
// / diegetic only).
//
// CTA_ANCHOR is the button's world seat; JourneyLine imports it so the two red
// journey rails converge their heads onto the button at t -> 1.
//
// ponytail: the graph-paper backdrop is a compact copy of the hero's shader
// rather than a shared component (avoids editing Hero mid-phase). Ceiling: two
// near-identical paper shaders. Upgrade path: lift GraphPaper to @/ink and import
// in both. ponytail: the CTA is a static paper mesh (no hover/press) -- P6 wires
// the real /download link; here it is only the diegetic target the rails land on.

import { useMemo } from "react";
import { Color, DoubleSide, Vector2 } from "three";
import { Line, Text } from "@react-three/drei";

import { finale } from "@/content/finale";
import InkStrokes from "@/ink/InkStrokes";
import { PALETTE } from "@/ink/palette";
import { charactersFor, FONT, FONT_TEXTS } from "@/ink/text";

const MONO_CHARS = charactersFor(...FONT_TEXTS.mono);
const HAND_CHARS = charactersFor(...FONT_TEXTS.hand);

// The CTA arrow is authored as a real U+2192 in content; Patrick Hand has no such
// glyph and ASCII-only source cannot type it, so strip the trailing arrow from
// the content string and spell it "->" (ruling: arrow as ASCII).
const CTA_LABEL = finale.cta.slice(0, -1) + "->";

// The button's world seat (the finale group sits at the origin, so local ==
// world). Exported so JourneyLine lands both rail heads onto the button at t -> 1.
export const CTA_ANCHOR: [number, number, number] = [0, -0.9, 0.3];

const BTN_W = 5.0;
const BTN_H = 0.95;
const HW = BTN_W / 2;
const HH = BTN_H / 2;
// Button border as a closed rectangle polyline (module-scope: never rebuilt).
const BTN_BORDER: [number, number, number][] = [
  [-HW, -HH, 0],
  [HW, -HH, 0],
  [HW, HH, 0],
  [-HW, HH, 0],
  [-HW, -HH, 0],
];
// Slightly ink-tinted paper so the button reads as a raised card.
const BTN_PAPER = new Color(PALETTE.paper).lerp(new Color(PALETTE.ink), 0.08);

// --- graph-paper ground: a compact echo of the hero desk. One draw, linear space
// (THREE.Color decodes the sRGB hex to linear for the post pipeline). fwidth
// anti-aliases the 1-unit ruling.
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
  vec2 c = vUv * uSize;
  vec2 g = abs( fract( c - 0.5 ) - 0.5 ) / fwidth( c );
  float ln = 1.0 - min( min( g.x, g.y ), 1.0 );
  gl_FragColor = vec4( mix( uPaper, uLine, ln * 0.55 ), 1.0 );
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

// --- CTA: a tilted paper button -- paper fill, ink border, Patrick Hand label.
function CtaButton() {
  return (
    <group position={CTA_ANCHOR} rotation={[0, 0, -0.03]}>
      <mesh>
        <planeGeometry args={[BTN_W, BTN_H]} />
        <meshBasicMaterial color={BTN_PAPER} side={DoubleSide} />
      </mesh>
      <Line points={BTN_BORDER} color={PALETTE.ink} worldUnits linewidth={0.035} />
      <Text
        font={FONT.hand}
        characters={HAND_CHARS}
        position={[0, 0, 0.05]}
        fontSize={0.34}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
      >
        {CTA_LABEL}
      </Text>
    </group>
  );
}

export default function Finale() {
  return (
    <group position={[0, 0, 0]}>
      <GraphPaper />

      {/* crown: the finale guy at the desk, hero-tier per-stroke, drawing on */}
      <InkStrokes name="finale-dood" position={[0, 5.6, 0.3]} scale={0.017} drawOn={{ t0: 0.885, t1: 0.92 }} />

      {/* wavy divider under the crown, drawing on at entry */}
      <InkStrokes name="inkline-1" position={[0, 3.55, 0.2]} scale={0.05} drawOn={{ t0: 0.882, t1: 0.9 }} />

      {/* the honest close (Space Mono, small -- lean body copy) */}
      <Text
        font={FONT.mono}
        characters={MONO_CHARS}
        position={[0, 3.0, 0.2]}
        fontSize={0.22}
        maxWidth={15}
        lineHeight={1.5}
        anchorX="center"
        anchorY="top"
        textAlign="center"
        color={PALETTE.ink}
      >
        {finale.honest}
      </Text>

      {/* the button the whole page walks toward -- where the rails land */}
      <CtaButton />

      {/* source, built-with, byline -- small footer lines */}
      <Text
        font={FONT.mono}
        characters={MONO_CHARS}
        position={[0, -2.5, 0.15]}
        fontSize={0.17}
        maxWidth={15}
        lineHeight={1.3}
        anchorX="center"
        anchorY="middle"
        textAlign="center"
        color={PALETTE.ink}
        fillOpacity={0.85}
      >
        {finale.srcGithub}
      </Text>
      <Text
        font={FONT.mono}
        characters={MONO_CHARS}
        position={[0, -3.2, 0.15]}
        fontSize={0.17}
        maxWidth={16}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
        fillOpacity={0.7}
      >
        {finale.builtwith}
      </Text>
      <Text
        font={FONT.mono}
        characters={MONO_CHARS}
        position={[0, -3.85, 0.15]}
        fontSize={0.18}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
      >
        {finale.byline}
      </Text>

      {/* the closing heart */}
      <InkStrokes name="inkline-2" position={[0, -4.9, 0.2]} scale={0.06} drawOn={{ t0: 0.95, t1: 0.98 }} />
    </group>
  );
}
