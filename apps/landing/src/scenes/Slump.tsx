"use client";

// P3 SLUMP -- the 2:47 AM room. Beat 2/8, t-range [0.125, 0.25], waypoint 1.
// The whole scene is authored in local units around the group origin, which is
// parked at world (1.8, -13.1, 0) -- the camera's look target at the mid-beat
// pose evalPose(0.1875). At that pose ~15 world units of height are visible, so
// everything readable is kept within roughly +/-7 of the origin; the content
// naturally sweeps up through frame as t moves off mid-beat (the plunge feel).
//
// Layers, back to front (+z toward camera):
//   - backdrop  : one darker paper quad (PALETTE.paper mixed ~20% toward ink) so
//                 the mood drops from the hero without leaving the sketchbook.
//   - room bars : skirting + two wall-edge strokes (thin ink boxes) for a
//                 pencil-margin room-corner feel.
//   - beat1-dood: the slumped guy at the monitor (desk rect + tear fill), a
//                 hero-tier InkStrokes with boil, drawn on as you enter the beat.
//   - cards     : the 5 screencap tabs as paper cards (ink-bordered quads + Space
//                 Mono), scattered/tilted, each fading in on its own t-window.
//   - deco      : deco-beat1-1..4 scribbles, staggered draw-on windows.
//   - GL text   : "2:47 AM" (Gloria) above; thought + counter (Patrick Hand)
//                 below. Counters show their FINAL values as static text (live
//                 ticking is a P6 gag).
//
// ponytail: the SMIL tear animation does not port -- the tear renders as a
// static fill. Swaying just the tear would need a per-stroke handle InkStrokes
// does not expose (it owns its objects); swaying the whole guy would be wrong.
// Ceiling: static tear. Upgrade path: a named sub-stroke ref out of InkStrokes.
// ponytail: all static ink outlines -- the room-corner scaffolding (skirting +
// two wall edges) AND the 5 card borders -- are merged into ONE drei <Line
// segments worldUnits> batch, built once (useMemo), no per-frame upload. This is
// Descent's proven draw-call pattern: it folds the 3 room-bar box meshes and the
// 5 per-card ink quads into a single draw (a ~7-draw cut in the hero+slump+
// descent mount overlap). Trade: the borders are now STATIC (no per-card fade),
// so a card's ink frame is drawn before its paper + text fade in on their f(t)
// window -- which reads fine: an empty inked frame is the sketch pre-drawing the
// screencap lands into. The fade writes only paper-quad opacity + text
// fillOpacity now.
// ponytail: beat1-dood fill polys (desk, tear) pop in at mount while the line
// strokes still draw on -- same at-rest-fill ceiling as Hero's hero-dood.

import { useMemo, useRef } from "react";
import { Color, type Group, type MeshBasicMaterial } from "three";
import { Line, Text } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";

import { beat1 } from "@/content/beat1";
import { journeyStore } from "@/engine/store";
import InkStrokes from "@/ink/InkStrokes";
import { PALETTE } from "@/ink/palette";
import { charactersFor, FONT, FONT_TEXTS } from "@/ink/text";

// Glyph unions, built once from the single-source registry so they stay warm
// with the preloadAllFonts() pass.
const SCRAWL_CHARS = charactersFor(...FONT_TEXTS.scrawl);
const MONO_CHARS = charactersFor(...FONT_TEXTS.mono);
const HAND_CHARS = charactersFor(...FONT_TEXTS.hand);

// Counter line, final values baked in (248 apps; the 248th was the crush, so 247
// unfortunatelies). Template pieces are imported already-escaped from content.
const COUNTER = beat1.counterA + "248" + beat1.counterB + "247" + beat1.counterC;

// Pure f(t): 0 before the window, 1 after -- scrubbing backwards un-reveals.
function progAt(t: number, t0: number, t1: number): number {
  const raw = (t - t0) / (t1 - t0);
  return raw < 0 ? 0 : raw > 1 ? 1 : raw;
}

interface CardProps {
  line: string;
  position: [number, number, number];
  rotation: number;
  win: [number, number];
}

// One screencap tab. A priority-0 useFrame maps t -> a fade progress and writes
// opacity onto the paper quad and the troika fill (fillOpacity is read every
// frame in troika's onBeforeRender, so no sync() is needed). The ink border is
// no longer a per-card quad -- it lives in the shared static Line batch below
// and stays drawn -- so the fade only touches paper opacity + text fillOpacity.
// Cheap uniform writes only; the fading paper + text are culled while hidden.
const CARD_W = 4.8;
const CARD_H = 1.5;
function ScreenCard({ line, position, rotation, win }: CardProps) {
  const groupRef = useRef<Group | null>(null);
  const paperRef = useRef<MeshBasicMaterial | null>(null);
  const textRef = useRef<{ fillOpacity: number } | null>(null);

  useFrame(() => {
    const p = progAt(journeyStore.getState().t, win[0], win[1]);
    if (groupRef.current) groupRef.current.visible = p > 0.001;
    if (paperRef.current) paperRef.current.opacity = p;
    if (textRef.current) textRef.current.fillOpacity = p;
  });

  return (
    <group ref={groupRef} position={position} rotation={[0, 0, rotation]} visible={false}>
      <mesh>
        <planeGeometry args={[CARD_W, CARD_H]} />
        <meshBasicMaterial ref={paperRef} color={PALETTE.paper} transparent opacity={0} />
      </mesh>
      <Text
        ref={textRef}
        font={FONT.mono}
        characters={MONO_CHARS}
        position={[0, 0, 0.02]}
        fontSize={0.15}
        maxWidth={CARD_W - 0.6}
        lineHeight={1.3}
        anchorX="center"
        anchorY="middle"
        textAlign="center"
        color={PALETTE.ink}
        fillOpacity={0}
      >
        {line}
      </Text>
    </group>
  );
}

// The 5 screencap tabs, scattered/tilted around the guy, staggered so all have
// drawn in by the mid-beat pose (~0.1875) when the scene reads centred.
const CARDS: CardProps[] = [
  { line: beat1.screencaps[0], position: [-6.9, 3.3, 0.2], rotation: 0.08, win: [0.132, 0.15] },
  { line: beat1.screencaps[1], position: [6.7, 3.0, 0.2], rotation: -0.07, win: [0.14, 0.158] },
  { line: beat1.screencaps[2], position: [-7.4, -0.6, 0.2], rotation: -0.05, win: [0.148, 0.166] },
  { line: beat1.screencaps[3], position: [7.2, -1.0, 0.2], rotation: 0.06, win: [0.156, 0.174] },
  { line: beat1.screencaps[4], position: [3.6, -4.8, 0.2], rotation: -0.04, win: [0.164, 0.182] },
];

// One merged fat-line batch: room-corner scaffolding (skirting + two wall edges)
// + the 5 static card borders. Built once (useMemo) so the drei <Line> uploads
// these buffers a single time -- no per-frame setPositions. Local coords, since
// the whole thing rides the root group at world (1.8, -13.1, 0). Card borders
// are the authored card layout (position + z rotation) traced as rectangles a
// hair in front of each fading paper quad.
const CARD_HW = CARD_W / 2;
const CARD_HH = CARD_H / 2;
function useInkBatch() {
  return useMemo(() => {
    const pts: [number, number, number][] = [];
    const seg = (ax: number, ay: number, az: number, bx: number, by: number, bz: number) => {
      pts.push([ax, ay, az], [bx, by, bz]);
    };
    // room corner: skirting + left + right wall edges (were 3 ink box meshes)
    seg(-13, -6.9, -0.85, 13, -6.9, -0.85);
    seg(-10.5, -6.8, -0.85, -10.5, 3.2, -0.85);
    seg(10.5, -6.8, -0.85, 10.5, 3.2, -0.85);
    // 5 card borders: rotated rectangles at the authored card layout (were the
    // outer ink quad of each 2-quad card).
    for (const c of CARDS) {
      const px = c.position[0];
      const py = c.position[1];
      const bz = c.position[2] + 0.015;
      const cos = Math.cos(c.rotation);
      const sin = Math.sin(c.rotation);
      const rx = (sx: number, sy: number) => px + sx * CARD_HW * cos - sy * CARD_HH * sin;
      const ry = (sx: number, sy: number) => py + sx * CARD_HW * sin + sy * CARD_HH * cos;
      seg(rx(-1, -1), ry(-1, -1), bz, rx(1, -1), ry(1, -1), bz);
      seg(rx(1, -1), ry(1, -1), bz, rx(1, 1), ry(1, 1), bz);
      seg(rx(1, 1), ry(1, 1), bz, rx(-1, 1), ry(-1, 1), bz);
      seg(rx(-1, 1), ry(-1, 1), bz, rx(-1, -1), ry(-1, -1), bz);
    }
    return pts;
  }, []);
}

export default function Slump() {
  // Darker paper: PALETTE.paper mixed ~20% toward ink (linear-space lerp -- both
  // Colors decode sRGB->linear on construction, matching the post pipeline).
  const darkPaper = useMemo(() => new Color(PALETTE.paper).lerp(new Color(PALETTE.ink), 0.2), []);
  const inkBatch = useInkBatch();

  return (
    <group position={[1.8, -13.1, 0]}>
      {/* darker paper backdrop */}
      <mesh position={[0, 0, -0.9]}>
        <planeGeometry args={[30, 20]} />
        <meshBasicMaterial color={darkPaper} />
      </mesh>

      {/* room-corner scaffolding + 5 card borders, merged into one batch (1 draw) */}
      <Line points={inkBatch} color={PALETTE.ink} segments worldUnits linewidth={0.05} />

      {/* the slumped guy: hero-tier per-stroke + boil, drawn on as you arrive */}
      <InkStrokes name="beat1-dood" position={[0, -1, 0]} scale={0.03} drawOn={{ t0: 0.128, t1: 0.17 }} />

      {/* deco scribbles: staggered draw-on across the beat slice */}
      <InkStrokes name="deco-beat1-1" position={[-9, 4.5, 0.05]} scale={0.02} drawOn={{ t0: 0.135, t1: 0.152 }} />
      <InkStrokes name="deco-beat1-2" position={[9, 4.2, 0.05]} scale={0.022} drawOn={{ t0: 0.145, t1: 0.162 }} />
      <InkStrokes name="deco-beat1-3" position={[9.5, -4.5, 0.05]} scale={0.024} drawOn={{ t0: 0.155, t1: 0.172 }} />
      <InkStrokes name="deco-beat1-4" position={[-9.5, -4, 0.05]} scale={0.03} drawOn={{ t0: 0.165, t1: 0.182 }} />

      {/* screencap tabs */}
      {CARDS.map((c, i) => (
        <ScreenCard key={i} {...c} />
      ))}

      {/* "2:47 AM" section label, small Gloria above the scene */}
      <Text
        font={FONT.scrawl}
        characters={SCRAWL_CHARS}
        position={[0, 5.2, 0.2]}
        fontSize={0.5}
        letterSpacing={0.04}
        anchorX="center"
        anchorY="middle"
        color={PALETTE.ink}
        fillOpacity={0.8}
      >
        {beat1.sectionLabel}
      </Text>

      {/* thought line */}
      <Text
        font={FONT.hand}
        characters={HAND_CHARS}
        position={[0, -5.8, 0.2]}
        fontSize={0.44}
        maxWidth={11}
        anchorX="center"
        anchorY="middle"
        textAlign="center"
        color={PALETTE.ink}
      >
        {beat1.thought}
      </Text>

      {/* counter line: FINAL values, static (live tick is a P6 gag) */}
      <Text
        font={FONT.hand}
        characters={HAND_CHARS}
        position={[0, -6.6, 0.2]}
        fontSize={0.3}
        maxWidth={12}
        lineHeight={1.2}
        anchorX="center"
        anchorY="middle"
        textAlign="center"
        color={PALETTE.ink}
        fillOpacity={0.72}
      >
        {COUNTER}
      </Text>
    </group>
  );
}
