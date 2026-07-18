"use client";

// P4 DEEP FRIED -- beat 4/8, t-range [0.375, 0.5], waypoint 3 (shaft bottom).
// The camera exits P3 just above the descent's black-hole disc (world y~=-42);
// crossing t=0.375 it BURSTS THROUGH into a different world. The whole beat is a
// pure function of the scroll-driven global t (journeyStore) -- scrub-safe both
// directions, no time-accumulated state -- so reversing un-does the burst, the
// draw-ons, the headline slams and the ray spin in exact mirror. This is the
// one loud set-piece: the fried post stack (Pass B) lands on top later; the
// scene here supplies the diegetic display moment (headline stays GL, ruling 2).
//
// Layers, back to front (+z toward camera), root parked at world (0,-43.2,0):
//   - Sunburst  : ONE custom-shader plane (linear-space) -- a saturated warm
//                 radial ground (#eec46c -> #ad5b0d) with alternating warm-white
//                 conic rays from a centre below-behind the guy. The ray phase is
//                 a pure f(t) slow spin; the whole plane bursts in (opacity +
//                 scale punch) across a tight window right at t=0.375, growing out
//                 of the portal to paint over the descent disc. 1 draw.
//   - guy       : beat3-dood, the grinning fried guy, hero-tier InkStrokes,
//                 centred and drawing on as the world bursts in.
//   - deco      : the 6 deco-beat3-* (bolts / star / flame) + stonks + crayon
//                 arrow, scattered with tight staggered drawOn windows early.
//   - headline  : "WAIT." / "IT DOES" / "EVERYTHING ELSE." as Anton lines, each
//                 SLAMMING in on its own staggered pure-f(t) scale(1.6->1.0) +
//                 opacity window. White fill, ink outline. Diegetic, stays GL.
//   - copy      : the mid line + the fried-line spine (ink) and its <b> emphasis
//                 (red) as two smaller Text blocks below.
//   - dialog    : the secret "Are you sure?" panel -- a static paper mesh with a
//                 title and two yes/YES button quads. Visual only (P6 wires it).
//
// ponytail: the fried-line's inline <b> spans render as ONE ink spine block plus
// ONE red emphasis block, not true inline-coloured rich text -- troika has no
// per-run colour and measuring word advances to hand-place segments is art-pass
// work. Ceiling: red bold sits in its own strip below the spine. Upgrade path:
// troika rich-text (or a manual glyph layout) when the fried copy gets its polish.
// ponytail: headline slams per LINE, not per word -- a per-word split needs troika
// word-advance measurement to re-flow "IT DOES" / "EVERYTHING ELSE." on one line;
// three staggered line-slams read as the same punch for a tenth of the plumbing.

import { useMemo, useRef } from "react";
import { Color, DoubleSide, type Group, Vector2 } from "three";
import { Line, Text } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";

import { beat3 } from "@/content/beat3";
import { journeyStore } from "@/engine/store";
import InkStrokes from "@/ink/InkStrokes";
import { PALETTE } from "@/ink/palette";
import { charactersFor, FONT, FONT_TEXTS } from "@/ink/text";

// Glyph unions built once from the single-source registry (stay warm with the
// preloadAllFonts() pass).
const IMPACT_CHARS = charactersFor(...FONT_TEXTS.impact);
const HAND_CHARS = charactersFor(...FONT_TEXTS.hand);
const MONO_CHARS = charactersFor(...FONT_TEXTS.mono);

// The fried line, imported (never retyped). Spine = the full sentence in reading
// order (ink, legible); bold = its <b> claims pulled into a red emphasis strip.
const L = beat3.line;
const FRIED_SPINE =
  L.p1 + L.b1 + L.p2 + L.b2 + L.p3 + L.b3 + L.p4 + L.b4 + L.p5 + L.b5 + L.p6;
const FRIED_BOLD = [L.b1, L.b2, L.b3, L.b4, L.b5].join("  .  ");

// Pure f(t): 0 before the window, 1 after -- scrubbing backwards un-reveals.
function progAt(t: number, t0: number, t1: number): number {
  const span = t1 - t0;
  const raw = span > 1e-6 ? (t - t0) / span : t >= t1 ? 1 : 0;
  return raw < 0 ? 0 : raw > 1 ? 1 : raw;
}

// --- the sunburst backdrop: warm radial ground + rotating conic rays, linear
// space (Color uniforms decode sRGB->linear on construction, matching the post
// pipeline -- same emit rule as Descent's grid/disc shaders). One draw call.
const SUN_VERT = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
}
`;
const SUN_FRAG = /* glsl */ `
uniform vec3 uInner;
uniform vec3 uOuter;
uniform vec3 uRay;
uniform vec2 uCenter;
uniform float uRays;
uniform float uRot;
uniform float uOpacity;
varying vec2 vUv;
void main() {
  vec2 d = vUv - uCenter;
  float r = length( d );
  vec3 ground = mix( uInner, uOuter, clamp( r * 1.7, 0.0, 1.0 ) );
  float ang = atan( d.y, d.x );
  float f = fract( ang / 6.2831853 * uRays + uRot );   // ray phase, spun by uRot
  float ray = smoothstep( 0.02, 0.10, f ) - smoothstep( 0.48, 0.56, f );
  float fade = smoothstep( 0.03, 0.16, r ) * ( 1.0 - smoothstep( 0.62, 1.0, r ) );
  vec3 col = mix( ground, uRay, clamp( ray, 0.0, 1.0 ) * 0.8 * fade );
  gl_FragColor = vec4( col, uOpacity );
}
`;

function Sunburst() {
  const grpRef = useRef<Group | null>(null);
  const uniforms = useMemo(
    () => ({
      uInner: { value: new Color("#eec46c") },
      uOuter: { value: new Color("#ad5b0d") },
      uRay: { value: new Color("#fff3d6") },
      uCenter: { value: new Vector2(0.5, 0.36) }, // ray origin: below-behind the guy
      uRays: { value: 16 },
      uRot: { value: 0 },
      uOpacity: { value: 0 },
    }),
    [],
  );

  useFrame(() => {
    const t = journeyStore.getState().t;
    const burst = progAt(t, 0.375, 0.402); // bursts through right at the crossing
    uniforms.uOpacity.value = burst;
    uniforms.uRot.value = (t - 0.375) * 0.9; // slow, scrub-safe spin
    if (grpRef.current) {
      grpRef.current.visible = burst > 0.003; // cull during the descent overlap
      grpRef.current.scale.setScalar(0.6 + 0.4 * burst); // punch out of the portal
    }
  });

  return (
    <group ref={grpRef} position={[0, 0.6, 0.5]} visible={false}>
      <mesh>
        <planeGeometry args={[40, 34]} />
        <shaderMaterial
          uniforms={uniforms}
          vertexShader={SUN_VERT}
          fragmentShader={SUN_FRAG}
          transparent
          depthWrite={false}
        />
      </mesh>
    </group>
  );
}

// --- one reveal-driven Text. drawOn-style pure f(t): a priority-0 useFrame maps
// t -> progress and writes it to the troika fill/outline opacity (read every
// frame in troika's onBeforeRender, no sync needed) plus a scale punch on the
// group. Culls itself to zero draws outside its window (matters in the descent
// overlap, where the headline sits pre-window). Outline gives the comic frame;
// punch>0 turns a fade into a slam.
type TextFade = { fillOpacity: number; outlineOpacity: number };
interface RevealTextProps {
  children: string;
  position: [number, number, number];
  fontFamily: string;
  characters: string;
  fontSize: number;
  win: [number, number];
  color: string;
  outline?: boolean;
  punch?: number;
  maxWidth?: number;
}
function RevealText({
  children,
  position,
  fontFamily,
  characters,
  fontSize,
  win,
  color,
  outline = false,
  punch = 0,
  maxWidth,
}: RevealTextProps) {
  const grpRef = useRef<Group | null>(null);
  const txtRef = useRef<TextFade | null>(null);

  useFrame(() => {
    const p = progAt(journeyStore.getState().t, win[0], win[1]);
    if (grpRef.current) {
      grpRef.current.visible = p > 0.001;
      if (punch) grpRef.current.scale.setScalar(1 + punch * (1 - p));
    }
    if (txtRef.current) {
      txtRef.current.fillOpacity = p;
      txtRef.current.outlineOpacity = p;
    }
  });

  return (
    <group ref={grpRef} position={position} visible={false}>
      <Text
        ref={txtRef}
        font={fontFamily}
        characters={characters}
        fontSize={fontSize}
        maxWidth={maxWidth}
        anchorX="center"
        anchorY="middle"
        textAlign="center"
        lineHeight={1.15}
        color={color}
        outlineColor={PALETTE.ink}
        outlineWidth={outline ? fontSize * 0.05 : 0}
        fillOpacity={0}
        outlineOpacity={0}
      >
        {children}
      </Text>
    </group>
  );
}

// --- the secret confirmation dialog: a static paper panel. Border rectangles
// (panel + 2 buttons) merge into ONE fat-line batch (1 draw), built once. The
// whole panel snaps in on a tight f(t) scale pop and culls itself outside the
// window (visual only -- P6 owns the interactivity).
const DLG_W = 3.3;
const DLG_H = 2.1;
const BTN_W = 1.35;
const BTN_H = 0.62;
const BTN_Y = -0.55;
const BTN_DX = 0.78;
function useDialogBorder() {
  return useMemo(() => {
    const pts: [number, number, number][] = [];
    const rect = (cx: number, cy: number, hw: number, hh: number, z: number) => {
      pts.push(
        [cx - hw, cy - hh, z], [cx + hw, cy - hh, z],
        [cx + hw, cy - hh, z], [cx + hw, cy + hh, z],
        [cx + hw, cy + hh, z], [cx - hw, cy + hh, z],
        [cx - hw, cy + hh, z], [cx - hw, cy - hh, z],
      );
    };
    rect(0, 0, DLG_W / 2, DLG_H / 2, 0.03);
    rect(-BTN_DX, BTN_Y, BTN_W / 2, BTN_H / 2, 0.03);
    rect(BTN_DX, BTN_Y, BTN_W / 2, BTN_H / 2, 0.03);
    return pts;
  }, []);
}

function DialogPanel() {
  const grpRef = useRef<Group | null>(null);
  const border = useDialogBorder();
  const btnPaper = useMemo(() => new Color(PALETTE.paper).lerp(new Color(PALETTE.ink), 0.1), []);

  useFrame(() => {
    const p = progAt(journeyStore.getState().t, 0.458, 0.478);
    if (grpRef.current) {
      grpRef.current.visible = p > 0.001;
      grpRef.current.scale.setScalar(1 + 0.14 * (1 - p));
    }
  });

  return (
    <group ref={grpRef} position={[4.9, -2.4, 1.05]} visible={false}>
      <mesh>
        <planeGeometry args={[DLG_W, DLG_H]} />
        <meshBasicMaterial color={PALETTE.paper} side={DoubleSide} />
      </mesh>
      <Line points={border} color={PALETTE.ink} segments worldUnits linewidth={0.03} />
      <mesh position={[-BTN_DX, BTN_Y, 0.02]}>
        <planeGeometry args={[BTN_W, BTN_H]} />
        <meshBasicMaterial color={btnPaper} side={DoubleSide} />
      </mesh>
      <mesh position={[BTN_DX, BTN_Y, 0.02]}>
        <planeGeometry args={[BTN_W, BTN_H]} />
        <meshBasicMaterial color={btnPaper} side={DoubleSide} />
      </mesh>
      <Text
        font={FONT.mono}
        characters={MONO_CHARS}
        position={[0, 0.62, 0.04]}
        fontSize={0.26}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
      >
        {beat3.dq}
      </Text>
      <Text
        font={FONT.mono}
        characters={MONO_CHARS}
        position={[-BTN_DX, BTN_Y, 0.04]}
        fontSize={0.22}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
      >
        {beat3.yes}
      </Text>
      <Text
        font={FONT.mono}
        characters={MONO_CHARS}
        position={[BTN_DX, BTN_Y, 0.04]}
        fontSize={0.22}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
      >
        {beat3.yes2}
      </Text>
    </group>
  );
}

// deco doodles: bolts / star / flame scatter around the guy, then the stonks
// chart + crayon arrow, each with a tight staggered draw-on window early in the
// beat. pos is authored world layout only (never time-animated).
const DECO: { name: string; pos: [number, number, number]; scale: number; win: [number, number] }[] = [
  { name: "deco-beat3-5", pos: [-3.6, 4.0, 0.8], scale: 0.035, win: [0.381, 0.395] },
  { name: "deco-beat3-6", pos: [3.8, 4.2, 0.8], scale: 0.035, win: [0.385, 0.399] },
  { name: "deco-beat3-1", pos: [-4.6, 2.2, 0.8], scale: 0.028, win: [0.379, 0.393] },
  { name: "deco-beat3-2", pos: [4.7, 2.6, 0.8], scale: 0.028, win: [0.383, 0.397] },
  { name: "deco-beat3-3", pos: [-5.0, -0.6, 0.8], scale: 0.03, win: [0.387, 0.401] },
  { name: "deco-beat3-4", pos: [5.1, -0.2, 0.8], scale: 0.03, win: [0.391, 0.405] },
  { name: "stonks", pos: [4.9, 0.9, 0.6], scale: 0.028, win: [0.398, 0.415] },
  { name: "crayon-arrow", pos: [-4.9, 0.6, 0.6], scale: 0.028, win: [0.401, 0.418] },
];

export default function Fried() {
  return (
    <group position={[0, -43.2, 0]}>
      {/* the burst-through world: warm radial + rotating rays, 1 draw */}
      <Sunburst />

      {/* the grinning fried guy: hero-tier per-stroke, drawing on as it bursts */}
      <InkStrokes name="beat3-dood" position={[0, 0.4, 0.7]} scale={0.018} drawOn={{ t0: 0.377, t1: 0.41 }} />

      {/* bolts / star / flame / stonks / arrow, staggered draw-on early */}
      {DECO.map((d) => (
        <InkStrokes key={d.name} name={d.name} position={d.pos} scale={d.scale} drawOn={{ t0: d.win[0], t1: d.win[1] }} />
      ))}

      {/* headline: three Anton lines, each SLAMS in (scale 1.6 -> 1.0 + opacity) */}
      <RevealText position={[0, 3.8, 1.2]} fontFamily={FONT.impact} characters={IMPACT_CHARS} fontSize={0.85} color="#ffffff" outline punch={0.6} win={[0.404, 0.422]}>
        {beat3.huge1}
      </RevealText>
      <RevealText position={[0, -2.4, 1.2]} fontFamily={FONT.impact} characters={IMPACT_CHARS} fontSize={0.9} color="#ffffff" outline punch={0.6} win={[0.42, 0.438]}>
        {beat3.huge2}
      </RevealText>
      <RevealText position={[0, -3.5, 1.2]} fontFamily={FONT.impact} characters={IMPACT_CHARS} fontSize={0.68} color="#ffffff" outline punch={0.6} maxWidth={11} win={[0.436, 0.454]}>
        {beat3.huge3}
      </RevealText>

      {/* mid line + fried-line spine (ink) + <b> emphasis strip (red), below */}
      <RevealText position={[0, -4.4, 0.9]} fontFamily={FONT.hand} characters={HAND_CHARS} fontSize={0.24} color={PALETTE.ink} maxWidth={10} win={[0.452, 0.472]}>
        {beat3.mid}
      </RevealText>
      <RevealText position={[0, -5.3, 0.9]} fontFamily={FONT.hand} characters={HAND_CHARS} fontSize={0.145} color={PALETTE.ink} maxWidth={13} win={[0.462, 0.486]}>
        {FRIED_SPINE}
      </RevealText>
      <RevealText position={[0, -6.15, 0.9]} fontFamily={FONT.hand} characters={HAND_CHARS} fontSize={0.15} color={PALETTE.red} maxWidth={12} win={[0.47, 0.492]}>
        {FRIED_BOLD}
      </RevealText>

      {/* the secret "Are you sure?" panel (visual only) */}
      <DialogPanel />
    </group>
  );
}
