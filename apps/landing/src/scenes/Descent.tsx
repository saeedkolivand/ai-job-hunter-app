"use client";

// P3 DESCENT -- beat 3/8, the plunge. The camera tips forward out of the slump
// room (journey waypoint 2, t=0.25) and falls a vertical shaft down to the
// black-hole disc just above the fried bottom (waypoint 3, t=0.375). All layout
// is a pure function of authored world position -- NOTHING here is time-driven,
// so the whole shaft is scrub-safe: the camera moves past static furniture, the
// furniture never moves. See engine/journey.ts for the matching camera pitch.
//
// Layers, back to front (+z toward camera):
//   - ShaftPaper : one tall graph-paper quad (Hero's linear-space grid shader),
//                  the receding paper the shaft is drawn on. One draw call.
//   - Walls      : one merged drei <Line segments> batch (worldUnits fat lines,
//                  built once, no per-frame upload) -- sparse vertical shaft
//                  edges, faint graph rungs, perspective guides converging on the
//                  disc, and every chip's red strike-through. One draw call.
//   - cards      : the 4 rejection-collage concepts from content/beat2 as tilted
//                  paper quads spiralling down at increasing depth (x alternates,
//                  y falls, z parallax) -- ink-outline border + one Space Mono /
//                  Patrick snippet each.
//   - chips      : the platform swarm as small Space Mono quads orbit-scattered
//                  on the shaft walls (strikes live in the Walls batch).
//   - beat2-dood : the frazzled guy mid-shaft, hero-tier per-stroke InkStrokes
//                  drawing on as the camera reaches him; deco-beat2-* scattered.
//   - BlackHole  : the dark radial-gradient disc at the shaft bottom (linear-
//                  space shader like ShaftPaper) with "HELLO??" (Anton) yelled
//                  around its rim. The camera LANDS just above it; the crash-
//                  through belongs to P4's fried beat.
//   - h2         : "the descent" (Gloria) at the shaft entry.
//
// ponytail: walls/borders/strikes are static fat lines (no line-boil) -- ceiling:
// they read a touch cleaner than the hand-inked hero strokes. Cheap on purpose.

import { useMemo } from "react";
import { Color, DoubleSide, Vector2 } from "three";
import { Line, Text } from "@react-three/drei";

import { beat2 } from "@/content/beat2";
import InkStrokes from "@/ink/InkStrokes";
import { PALETTE } from "@/ink/palette";
import { charactersFor, FONT, FONT_TEXTS } from "@/ink/text";

// --- shared linear-space grid shader (same emit rule as Hero's GraphPaper) ---
const GRID_VERT = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
}
`;
const GRID_FRAG = /* glsl */ `
uniform vec3 uPaper;
uniform vec3 uLine;
uniform vec2 uSize;
varying vec2 vUv;
void main() {
  vec2 c = vUv * uSize;                       // 1 cell == 1 world unit
  vec2 g = abs( fract( c - 0.5 ) - 0.5 ) / fwidth( c );
  float ln = 1.0 - min( min( g.x, g.y ), 1.0 );
  vec3 col = mix( uPaper, uLine, ln * 0.5 );
  gl_FragColor = vec4( col, 1.0 );
}
`;

const SHAFT_W = 26;
const SHAFT_H = 34;

function ShaftPaper() {
  const uniforms = useMemo(
    () => ({
      uPaper: { value: new Color(PALETTE.paper) },
      uLine: { value: new Color(PALETTE.line) },
      uSize: { value: new Vector2(SHAFT_W, SHAFT_H) },
    }),
    [],
  );
  return (
    <mesh position={[0, -28, -1.2]}>
      <planeGeometry args={[SHAFT_W, SHAFT_H]} />
      <shaderMaterial uniforms={uniforms} vertexShader={GRID_VERT} fragmentShader={GRID_FRAG} />
    </mesh>
  );
}

// --- the black-hole disc: dark radial gradient, linear-space, rim fades to paper.
const DISC_FRAG = /* glsl */ `
uniform vec3 uInner;
uniform vec3 uOuter;
uniform vec3 uRing;
varying vec2 vUv;
void main() {
  float d = length( vUv - 0.5 ) * 2.0;         // 0 at core .. 1 at rim
  vec3 col = mix( uInner, uOuter, pow( clamp( d, 0.0, 1.0 ), 1.7 ) );
  float ring = smoothstep( 0.6, 0.68, d ) * ( 1.0 - smoothstep( 0.68, 0.78, d ) );
  col = mix( col, uRing, ring * 0.55 );
  float alpha = 1.0 - smoothstep( 0.88, 1.0, d );
  gl_FragColor = vec4( col, alpha );
}
`;

function BlackHole() {
  const uniforms = useMemo(
    () => ({
      uInner: { value: new Color("#0a0806") },
      uOuter: { value: new Color(PALETTE.paper) },
      uRing: { value: new Color(PALETTE.red) },
    }),
    [],
  );
  return (
    <mesh position={[0, -42, 0]} renderOrder={0}>
      <circleGeometry args={[5, 48]} />
      <shaderMaterial
        uniforms={uniforms}
        vertexShader={GRID_VERT}
        fragmentShader={DISC_FRAG}
        transparent
        depthWrite={false}
      />
    </mesh>
  );
}

// --- chip swarm: platform names X'd out, orbit-scattered on the shaft walls ---
// [label, x, y]. First 6 of beat2.chips; strike diagonals feed the Walls batch.
const CHIPS: [string, number, number][] = [
  [beat2.chips[0], -5.2, -17],
  [beat2.chips[1], 5.0, -20.5],
  [beat2.chips[2], -5.6, -24.5],
  [beat2.chips[3], 5.4, -29],
  [beat2.chips[4], -5.0, -34],
  [beat2.chips[5], 4.8, -38],
];

// --- rejection cards: the 4 collage concepts, spiralling down at rising depth.
// pos is authored world layout only (never time-animated). [text, fontKey,
// color, x, y, z, rotZ, w, h, fontSize, maxWidth]
const CARDS: {
  text: string;
  fontKey: keyof typeof FONT;
  color: string;
  pos: [number, number, number];
  rot: number;
  w: number;
  h: number;
  fs: number;
  maxW: number;
}[] = [
  { text: beat2.blackholeMain, fontKey: "monoBold", color: PALETTE.ink, pos: [-3.0, -17.5, 0.2], rot: 0.1, w: 4.4, h: 2.8, fs: 0.3, maxW: 3.7 },
  { text: beat2.atsBold, fontKey: "mono", color: PALETTE.red, pos: [3.1, -22.5, 0.4], rot: -0.14, w: 4.0, h: 2.2, fs: 0.5, maxW: 3.5 },
  { text: beat2.recruitersTitle, fontKey: "mono", color: PALETTE.ink, pos: [-3.3, -30.5, 0.6], rot: 0.15, w: 4.2, h: 2.4, fs: 0.34, maxW: 3.5 },
  { text: beat2.linkedinTitle, fontKey: "hand", color: PALETTE.ink, pos: [3.0, -35, 0.5], rot: -0.1, w: 4.4, h: 2.4, fs: 0.4, maxW: 3.7 },
];

// One merged fat-line batch: shaft edges + faint graph rungs + perspective
// guides converging on the disc + chip strikes. Built once (useMemo), so no
// per-frame setPositions -- the drei <Line> uploads these buffers a single time.
function useWalls() {
  return useMemo(() => {
    const INK = new Color(PALETTE.ink);
    const FAINT = new Color(PALETTE.line);
    const RED = new Color(PALETTE.red);
    const points: [number, number, number][] = [];
    const colors: Color[] = [];
    const seg = (
      ax: number, ay: number, bx: number, by: number, c: Color,
    ) => {
      points.push([ax, ay, 0], [bx, by, 0]);
      colors.push(c, c);
    };
    // broken vertical shaft edges (sketchbook look), both walls
    seg(-6, -12, -6, -28, INK);
    seg(-6, -31, -6, -43, INK);
    seg(-5, -15, -5, -26, INK);
    seg(6, -12, 6, -28, INK);
    seg(6, -31, 6, -43, INK);
    seg(5, -16, 5, -27, INK);
    // perspective guides converging on the disc -- sells the drop
    seg(-6, -13, -1.2, -40, INK);
    seg(6, -13, 1.2, -40, INK);
    seg(-5, -26, -0.8, -41, FAINT);
    seg(5, -26, 0.8, -41, FAINT);
    // faint graph rungs receding down
    seg(-6, -20, -3, -20, FAINT);
    seg(3, -20, 6, -20, FAINT);
    seg(-6, -32, -3, -32, FAINT);
    seg(3, -32, 6, -32, FAINT);
    // chip strike-throughs (red)
    for (const [, x, y] of CHIPS) seg(x - 1.1, y - 0.22, x + 1.1, y + 0.22, RED);
    return { points, colors };
  }, []);
}

export default function Descent() {
  const walls = useWalls();
  return (
    <group>
      <ShaftPaper />

      {/* shaft edges + graph hints + perspective guides + chip strikes (1 batch) */}
      <Line points={walls.points} vertexColors={walls.colors} segments worldUnits linewidth={0.055} />

      {/* h2 at the shaft entry */}
      <Text
        font={FONT.scrawl}
        characters={charactersFor(...FONT_TEXTS.scrawl)}
        position={[-0.3, -15.5, 0.4]}
        rotation={[0, 0, 0.03]}
        fontSize={1.0}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
      >
        {beat2.h2}
      </Text>

      {/* rejection cards: static spiral, camera falls past them */}
      {CARDS.map((c, i) => (
        <group key={i} position={c.pos} rotation={[0, 0, c.rot]}>
          <mesh>
            <planeGeometry args={[c.w, c.h]} />
            <meshBasicMaterial color={PALETTE.paper} side={DoubleSide} />
          </mesh>
          <Line
            points={[
              [-c.w / 2, -c.h / 2, 0.01],
              [c.w / 2, -c.h / 2, 0.01],
              [c.w / 2, c.h / 2, 0.01],
              [-c.w / 2, c.h / 2, 0.01],
              [-c.w / 2, -c.h / 2, 0.01],
            ]}
            color={PALETTE.ink}
            worldUnits
            linewidth={0.04}
          />
          <Text
            font={FONT[c.fontKey]}
            characters={charactersFor(c.text)}
            position={[0, 0, 0.06]}
            fontSize={c.fs}
            maxWidth={c.maxW}
            lineHeight={1.15}
            anchorX="center"
            anchorY="middle"
            textAlign="center"
            color={c.color}
          >
            {c.text}
          </Text>
        </group>
      ))}

      {/* chip swarm: platform names, X'd out (strikes are in the Walls batch) */}
      {CHIPS.map(([label, x, y], i) => (
        <Text
          key={i}
          font={FONT.mono}
          characters={charactersFor(...FONT_TEXTS.mono)}
          position={[x, y, 0.15]}
          fontSize={0.32}
          anchorX="center"
          anchorY="middle"
          color={PALETTE.ink}
          fillOpacity={0.7}
        >
          {label}
        </Text>
      ))}
      <Text
        font={FONT.mono}
        characters={charactersFor(...FONT_TEXTS.mono)}
        position={[4.2, -41, 0.15]}
        fontSize={0.3}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
        fillOpacity={0.55}
      >
        {beat2.chipsMore}
      </Text>

      {/* frazzled guy mid-shaft (hero tier) + scattered deco */}
      <InkStrokes name="beat2-dood" position={[0.2, -26.5, 0.7]} scale={0.022} drawOn={{ t0: 0.275, t1: 0.32 }} />
      <InkStrokes name="deco-beat2-1" position={[-5.0, -19, 0.1]} scale={0.03} drawOn={{ t0: 0.255, t1: 0.285 }} />
      <InkStrokes name="deco-beat2-3" position={[4.6, -27, 0.1]} scale={0.035} drawOn={{ t0: 0.29, t1: 0.32 }} />
      <InkStrokes name="deco-beat2-6" position={[-4.4, -38, 0.1]} scale={0.03} drawOn={{ t0: 0.33, t1: 0.36 }} />

      {/* the black hole at the shaft bottom + HELLO?? yelled around its rim */}
      <BlackHole />
      <Text
        font={FONT.impact}
        characters={charactersFor(...FONT_TEXTS.impact)}
        position={[0, -38.2, 0.3]}
        fontSize={0.85}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.red}
      >
        {beat2.blackholeYell}
      </Text>
      <Text
        font={FONT.impact}
        characters={charactersFor(...FONT_TEXTS.impact)}
        position={[-3.4, -41.6, 0.3]}
        rotation={[0, 0, 0.32]}
        fontSize={0.66}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.red}
        fillOpacity={0.85}
      >
        {beat2.blackholeYell}
      </Text>
      <Text
        font={FONT.impact}
        characters={charactersFor(...FONT_TEXTS.impact)}
        position={[3.2, -42.8, 0.3]}
        rotation={[0, 0, -0.32]}
        fontSize={0.66}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.red}
        fillOpacity={0.7}
      >
        {beat2.blackholeYell}
      </Text>
    </group>
  );
}
