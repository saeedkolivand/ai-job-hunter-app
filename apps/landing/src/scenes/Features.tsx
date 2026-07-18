"use client";

// P5 FEATURES -- beat 6/8, the paper-card canyon. t-range [0.625, 0.75],
// waypoint 6->7. The camera exits the godmode plateau (0,20,14) looking straight
// down -z, then pans right and dollies +x to (10,19,10) looking toward (22,19,0)
// -- so the beat is a rightward corridor at y~=20, z~=0 (positions read from
// journey.ts + sampled through evalPose). The 11 feature cards line that corridor
// as two facing rows (upper y=21.6, lower y=19.0, alternating) the lens travels
// between; each card yaws to face the camera at the moment it is nearest, and
// stagger-reveals as the camera approaches (pure f(t) windows keyed off the
// look-target x reaching each card, so scrubbing back un-draws in exact mirror).
//
// Layout, back to front:
//   - borders : ALL 11 card rectangles pre-transformed (yaw+translate) into ONE
//               merged fat-line batch (1 draw) -- the panels are "already inked
//               on the page"; contents fill in as you arrive.
//   - cards    : per card a paper quad + Patrick Hand title + smaller Patrick body,
//               opacity-ramped over the card's window (title always legible; body
//               is lean + clipRect'd to the card -- the art pass redoes typography
//               and moves body copy to a DOM overlay, ruling 2).
//   - entry    : h2 "what it actually does" (Gloria) + lede (Patrick) at the canyon
//               mouth, with crayon-arrow drawing on to point into the corridor.
//   - deco     : four deco-features doodles frame the ceiling/floor, staggered.
//
// The whole scene culls (root visible=false) outside a tight t-window so it
// submits ZERO draws while it is only a co-mounted neighbour of godmode /
// testimonials. In-beat peak ~48 draws (33 card + 1 border + ~12 deco + entry).
//
// ponytail: staging is lean. Borders are static (not per-card draw-on) so the
// budget stays at one merged batch -- the stagger is carried by the card fill +
// text opacity instead. GL body copy is display-only here (the pattern in every
// other beat); the art pass owns the DOM-overlay move. Card yaw faces a single
// sampled camera pose per card, not the live camera -- good enough while the lens
// only pans ~40deg across the beat. Ceilings, all of it.

import { useMemo, useRef } from "react";
import type { Group, MeshBasicMaterial } from "three";
import { Line, Text } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";

import { features } from "@/content/features";
import { journeyStore } from "@/engine/store";
import InkStrokes from "@/ink/InkStrokes";
import { PALETTE } from "@/ink/palette";
import { charactersFor, FONT, FONT_TEXTS } from "@/ink/text";

const HAND_CHARS = charactersFor(...FONT_TEXTS.hand);
const SCRAWL_CHARS = charactersFor(...FONT_TEXTS.scrawl);

// Card geometry (world units). Half extents drive both the paper quad and the
// merged border, so they stay locked together.
const HW = 1.35;
const HH = 1.15;

// Pure f(t): 0 before the window, 1 after -- scrubbing backwards un-reveals.
function progAt(t: number, t0: number, t1: number): number {
  const span = t1 - t0;
  const raw = span > 1e-6 ? (t - t0) / span : t >= t1 ? 1 : 0;
  return raw < 0 ? 0 : raw > 1 ? 1 : raw;
}

// The 11 cards, in content order, laid left->right down the corridor. x/y are the
// world card centre; yaw faces the sampled camera pose when the look-target x
// first reaches the card; win is the stagger-reveal window (finishes just before
// the lens arrives, so the card is settled and legible on approach). Body folds
// any inline emphasis into one plain hand string (troika has no per-run colour --
// the art pass owns rich text). c11 renders the link labels as plain ink; GL has
// no anchors, the DOM overlay wires them later.
interface CardDef {
  t: string;
  p: string;
  x: number;
  y: number;
  yaw: number;
  win: [number, number];
}
const CARDS: CardDef[] = [
  { t: features.c1t, p: features.c1p, x: 4.5, y: 21.6, yaw: -0.197, win: [0.632, 0.658] },
  { t: features.c2t, p: features.c2p, x: 6.0, y: 19.0, yaw: -0.27, win: [0.64, 0.666] },
  { t: features.c3t, p: features.c3p, x: 7.5, y: 21.6, yaw: -0.344, win: [0.647, 0.673] },
  { t: features.c4t, p: features.c4p, x: 9.0, y: 19.0, yaw: -0.416, win: [0.653, 0.679] },
  { t: features.c5t, p: features.c5p, x: 10.5, y: 21.6, yaw: -0.486, win: [0.66, 0.686] },
  { t: features.c6t, p: features.c6p, x: 12.0, y: 19.0, yaw: -0.554, win: [0.666, 0.692] },
  { t: features.c7t, p: features.c7p, x: 13.5, y: 21.6, yaw: -0.617, win: [0.673, 0.699] },
  { t: features.c8t, p: features.c8p, x: 15.0, y: 19.0, yaw: -0.677, win: [0.68, 0.706] },
  { t: features.c9t, p: features.c9pA + features.c9pI + features.c9pB, x: 16.5, y: 21.6, yaw: -0.731, win: [0.687, 0.713] },
  { t: features.c10t, p: features.c10p, x: 18.0, y: 19.0, yaw: -0.781, win: [0.694, 0.72] },
  {
    t: features.c11t,
    p: features.c11pA + features.c11Chrome + features.c11Sep + features.c11Firefox + features.c11pEnd,
    x: 19.5,
    y: 21.6,
    yaw: -0.824,
    win: [0.702, 0.728],
  },
];

// deco-features garnish framing the corridor ceiling/floor, staggered draw-on.
const DECO: { name: string; pos: [number, number, number]; scale: number; win: [number, number] }[] = [
  { name: "deco-features-1", pos: [5.2, 22.7, 0.5], scale: 0.02, win: [0.645, 0.664] },
  { name: "deco-features-2", pos: [9.7, 17.9, 0.5], scale: 0.02, win: [0.672, 0.69] },
  { name: "deco-features-3", pos: [14.2, 22.7, 0.5], scale: 0.02, win: [0.697, 0.714] },
  { name: "deco-features-4", pos: [18.7, 17.9, 0.5], scale: 0.02, win: [0.71, 0.727] },
];

// One merged fat-line batch: every card rectangle, corners rotated by the card's
// yaw and translated to its centre so the static border tracks the yawed quad.
// Built once; drei <Line ... segments> = a single LineSegments2 draw call.
function useCardBorders() {
  return useMemo(() => {
    const pts: [number, number, number][] = [];
    for (const c of CARDS) {
      const s = Math.sin(c.yaw);
      const cs = Math.cos(c.yaw);
      // rotate (lx,ly,0) about +y then translate to (c.x,c.y,0): z = -lx*sin.
      const corner = (lx: number, ly: number): [number, number, number] => [
        c.x + lx * cs,
        c.y + ly,
        -lx * s,
      ];
      const a = corner(-HW, -HH);
      const b = corner(HW, -HH);
      const d = corner(HW, HH);
      const e = corner(-HW, HH);
      pts.push(a, b, b, d, d, e, e, a);
    }
    return pts;
  }, []);
}

// Per-card reveal: paper fill + title + body opacity ramp over the card window,
// group culled before it opens (so unrevealed cards cost zero draws). No scale
// punch -- the border is a separate static batch and must stay aligned.
type TextFade = { fillOpacity: number };
function Card({ def }: { def: CardDef }) {
  const grpRef = useRef<Group | null>(null);
  const paperRef = useRef<MeshBasicMaterial | null>(null);
  const titleRef = useRef<TextFade | null>(null);
  const bodyRef = useRef<TextFade | null>(null);

  useFrame(() => {
    const p = progAt(journeyStore.getState().t, def.win[0], def.win[1]);
    if (grpRef.current) grpRef.current.visible = p > 0.001;
    if (paperRef.current) paperRef.current.opacity = p * 0.96;
    if (titleRef.current) titleRef.current.fillOpacity = p;
    if (bodyRef.current) bodyRef.current.fillOpacity = p;
  });

  return (
    <group ref={grpRef} position={[def.x, def.y, 0]} rotation={[0, def.yaw, 0]} visible={false}>
      <mesh>
        <planeGeometry args={[HW * 2, HH * 2]} />
        <meshBasicMaterial ref={paperRef} color={PALETTE.paper} side={2} transparent opacity={0} depthWrite={false} />
      </mesh>
      <Text
        ref={titleRef}
        font={FONT.hand}
        characters={HAND_CHARS}
        position={[0, 0.98, 0.02]}
        fontSize={0.19}
        maxWidth={2.5}
        lineHeight={1.05}
        anchorX="center"
        anchorY="top"
        textAlign="center"
        color={PALETTE.ink}
        fillOpacity={0}
      >
        {def.t}
      </Text>
      <Text
        ref={bodyRef}
        font={FONT.hand}
        characters={HAND_CHARS}
        position={[0, 0.24, 0.02]}
        fontSize={0.1}
        maxWidth={2.45}
        lineHeight={1.16}
        anchorX="center"
        anchorY="top"
        textAlign="center"
        color={PALETTE.ink}
        clipRect={[-1.28, -1.55, 1.28, 0.06]}
        fillOpacity={0}
      >
        {def.p}
      </Text>
    </group>
  );
}

// Entry copy (h2 + lede): one shared opacity-ramped Text, culled outside window.
function RevealText({
  children,
  position,
  font,
  characters,
  fontSize,
  color,
  win,
  maxWidth,
}: {
  children: string;
  position: [number, number, number];
  font: string;
  characters: string;
  fontSize: number;
  color: string;
  win: [number, number];
  maxWidth: number;
}) {
  const grpRef = useRef<Group | null>(null);
  const txtRef = useRef<TextFade | null>(null);
  useFrame(() => {
    const p = progAt(journeyStore.getState().t, win[0], win[1]);
    if (grpRef.current) grpRef.current.visible = p > 0.001;
    if (txtRef.current) txtRef.current.fillOpacity = p;
  });
  return (
    <group ref={grpRef} position={position} visible={false}>
      <Text
        ref={txtRef}
        font={font}
        characters={characters}
        fontSize={fontSize}
        maxWidth={maxWidth}
        anchorX="center"
        anchorY="middle"
        textAlign="center"
        lineHeight={1.15}
        color={color}
        fillOpacity={0}
      >
        {children}
      </Text>
    </group>
  );
}

export default function Features() {
  const rootRef = useRef<Group | null>(null);
  const border = useCardBorders();

  // Cull the whole beat outside a tight t-window: it is a co-mounted neighbour of
  // godmode (active 0.5-0.625) and testimonials (active 0.75-0.875), and must
  // submit no draws there. Pure f(t) -- the flip is deterministic and scrub-safe.
  useFrame(() => {
    const t = journeyStore.getState().t;
    if (rootRef.current) rootRef.current.visible = t >= 0.62 && t <= 0.775;
  });

  return (
    <group ref={rootRef} visible={false}>
      {/* the panels, pre-inked: one merged border batch (1 draw) */}
      <Line points={border} segments worldUnits linewidth={0.02} color={PALETTE.ink} renderOrder={3} />

      {/* canyon entry: h2 (Gloria) + lede (Patrick), then the arrow pointing in */}
      <RevealText position={[1.8, 21.5, 0.2]} font={FONT.scrawl} characters={SCRAWL_CHARS} fontSize={0.52} color={PALETTE.ink} win={[0.626, 0.65]} maxWidth={6}>
        {features.h2}
      </RevealText>
      <RevealText position={[1.8, 20.5, 0.2]} font={FONT.hand} characters={HAND_CHARS} fontSize={0.26} color={PALETTE.ink} win={[0.632, 0.656]} maxWidth={6}>
        {features.lede}
      </RevealText>
      <InkStrokes name="crayon-arrow" position={[3.4, 20.2, 0.3]} scale={0.02} drawOn={{ t0: 0.634, t1: 0.656 }} />

      {/* the two facing rows of feature cards */}
      {CARDS.map((c) => (
        <Card key={c.t} def={c} />
      ))}

      {/* corridor garnish, staggered draw-on */}
      {DECO.map((d) => (
        <InkStrokes key={d.name} name={d.name} position={d.pos} scale={d.scale} drawOn={{ t0: d.win[0], t1: d.win[1] }} />
      ))}
    </group>
  );
}
