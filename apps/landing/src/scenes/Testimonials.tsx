"use client";

// P5 TESTIMONIALS -- beat 7/8, the pinboard wall. t-range [0.75, 0.875],
// waypoint 7->8. The camera glides right off the features corridor and settles
// dead-on the wall at pose (position (26,17,12), look (26,17,0)) -- distance 12,
// fov 75, the same head-on framing Hero uses (~18.4 world units of visible
// height at the z=0 page). So the whole scene is authored in local units around
// a group parked at world (26, 17, 0), the look target. All layout is pure
// authored world position; only the reveal windows are f(t), so the beat scrubs
// clean both directions (a note un-pins as you scroll back).
//
// Layers, back to front (+z toward camera):
//   - backdrop : one paper page quad, the pinboard the notes sit on.
//   - borders  : all 9 card frames baked (tilt + position) into ONE Line2
//                segments batch -- one draw call for every ink border (ruling 9:
//                density via merged batches).
//   - cards    : 9 tilted paper notes in a masonry-ish 3x3, each a paper quad +
//                Patrick Hand quote + Gloria attribution, fading up (paper, then
//                text chasing) on its own staggered window. One note carries the
//                star rating (ASCII asterisks -- the unicode star is not in the
//                GL atlas; see scrubEmoji in ink/text.ts).
//   - deco     : star scribbles + quote curls + a thumbs-up (deco-testi-*)
//                boiling beside the wall, each on a staggered draw-on window.
//   - GL text  : h2 crown "what the people are saying" (top, largest) + the
//                "(the people are not real)" sub-label; the "as featured in..."
//                Space Mono line last and smallest at the axis bottom (crown
//                rule: heading top, wall below, featured last/smallest).
//
// ponytail: staging is lean -- borders are a static merged batch (no per-stroke
// draw-on) and card text is world-space GL, always drawn while mounted; the
// stagger is carried by paper+text opacity, not a dashed stroke reveal. Ceiling:
// the art pass moves body copy to a DOM overlay + owns true stroke draw-on.

import { useMemo, useRef } from "react";
import { DoubleSide, type Group, type MeshBasicMaterial } from "three";
import { Line, Text } from "@react-three/drei";
import { useFrame } from "@react-three/fiber";

import { testimonials } from "@/content/testimonials";
import { journeyStore } from "@/engine/store";
import InkStrokes from "@/ink/InkStrokes";
import { PALETTE } from "@/ink/palette";
import { charactersFor, FONT, FONT_TEXTS } from "@/ink/text";

// Family-wide glyph unions, built once from the single-source registry so the
// atlas matches preloadAllFonts() exactly (no throwaway second atlas).
const HAND_CHARS = charactersFor(...FONT_TEXTS.hand);
const SCRAWL_CHARS = charactersFor(...FONT_TEXTS.scrawl);
const MONO_CHARS = charactersFor(...FONT_TEXTS.mono);

const CARD_PAPER = "#fffdf6"; // note stock (matches .quote in globals.css)
const WHO_COLOR = "#7a6f55"; // attribution ink
const STAR_GOLD = "#e0a32a"; // the star rating
const STARS = "*****"; // ASCII rating -- NO unicode star glyph (not in the atlas)
const DEG = Math.PI / 180;

// Featured line composed once from imported pieces (never retyped), rendered as
// one Space Mono block. The <b> emphasis is polish for the art pass.
const FEATURED = testimonials.featuredPrefix + testimonials.featured.join(testimonials.sep);

// Pure f(t): 0 before the window, 1 after -- scrubbing backwards un-reveals.
function progAt(t: number, t0: number, t1: number): number {
  const span = t1 - t0;
  const raw = span > 1e-6 ? (t - t0) / span : t >= t1 ? 1 : 0;
  return raw < 0 ? 0 : raw > 1 ? 1 : raw;
}

// --- one card note: a tilted paper quad + quote + attribution, all revealing on
// a per-card window. paper fades first, text chases ~30% behind (color chases
// line, art brief section 2). Culls to zero draws before its window. ---
type TextFade = { fillOpacity: number };
interface CardCfg {
  quote: string;
  who: string;
  cx: number;
  cy: number;
  w: number;
  h: number;
  tilt: number;
  win: [number, number];
  quoteColor: string;
  quoteSize: number;
}

function Card({ quote, who, cx, cy, w, h, tilt, win, quoteColor, quoteSize }: CardCfg) {
  const grpRef = useRef<Group | null>(null);
  const paperRef = useRef<MeshBasicMaterial | null>(null);
  const qRef = useRef<TextFade | null>(null);
  const wRef = useRef<TextFade | null>(null);

  useFrame(() => {
    const p = progAt(journeyStore.getState().t, win[0], win[1]);
    if (grpRef.current) grpRef.current.visible = p > 0.001;
    if (paperRef.current) paperRef.current.opacity = p;
    const tp = Math.min(1, Math.max(0, (p - 0.3) / 0.6)); // text chases paper
    if (qRef.current) qRef.current.fillOpacity = tp;
    if (wRef.current) wRef.current.fillOpacity = tp;
  });

  const padX = -(w / 2) + 0.28;
  return (
    <group ref={grpRef} position={[cx, cy, 0]} rotation={[0, 0, tilt]} visible={false}>
      <mesh>
        <planeGeometry args={[w, h]} />
        <meshBasicMaterial ref={paperRef} color={CARD_PAPER} side={DoubleSide} transparent opacity={0} />
      </mesh>
      <Text
        ref={qRef}
        font={FONT.hand}
        characters={HAND_CHARS}
        position={[padX, h / 2 - 0.34, 0.1]}
        fontSize={quoteSize}
        maxWidth={w - 0.56}
        lineHeight={1.18}
        anchorX="left"
        anchorY="top"
        textAlign="left"
        color={quoteColor}
        fillOpacity={0}
      >
        {quote}
      </Text>
      <Text
        ref={wRef}
        font={FONT.scrawl}
        characters={SCRAWL_CHARS}
        position={[padX, -(h / 2) + 0.32, 0.1]}
        fontSize={0.2}
        maxWidth={w - 0.5}
        anchorX="left"
        anchorY="middle"
        textAlign="left"
        color={WHO_COLOR}
        fillOpacity={0}
      >
        {who}
      </Text>
    </group>
  );
}

// --- a fading GL label (heading / sub-label / featured line). ---
interface RevealTextProps {
  children: string;
  position: [number, number, number];
  font: string;
  characters: string;
  fontSize: number;
  maxWidth?: number;
  color: string;
  win: [number, number];
}
function RevealText({ children, position, font, characters, fontSize, maxWidth, color, win }: RevealTextProps) {
  const grpRef = useRef<Group | null>(null);
  const txtRef = useRef<TextFade | null>(null);
  useFrame(() => {
    const p = progAt(journeyStore.getState().t, win[0], win[1]);
    if (grpRef.current) grpRef.current.visible = p > 0.001;
    if (txtRef.current) txtRef.current.fillOpacity = p;
  });
  return (
    <group ref={grpRef} visible={false}>
      <Text
        ref={txtRef}
        font={font}
        characters={characters}
        position={position}
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

// masonry-ish 3x3 blocking: fixed columns/rows + per-card y jitter + tilt read as
// a pinned board (ruling 7: tilt not sprawl). Heights vary with quote length so
// the columns pack unevenly. All authored layout -- never time-animated.
const COLS = [-4.9, 0, 4.9];
const ROWS = [2.9, 0, -2.9];
const JIT = [0.25, -0.2, 0.3, -0.15, 0.2, -0.25, 0.15, -0.2, 0.25];
const TILT = [-2.2, 1.5, -1.6, 2.4, -2.0, 1.8, -1.4, 2.2, -2.6];
const HEIGHTS = [1.7, 1.7, 2.4, 2.1, 2.1, 1.7, 2.6, 1.6, 1.7];
const CARD_W = 3.5;
const winFor = (i: number): [number, number] => [0.79 + i * 0.007, 0.814 + i * 0.007];

// 8 quote notes + 1 star-rating note = the 9-card wall.
const CARDS: CardCfg[] = testimonials.quotes.map((q, i) => ({
  quote: q.quote,
  who: q.who,
  cx: COLS[i % 3] ?? 0,
  cy: (ROWS[Math.floor(i / 3)] ?? 0) + (JIT[i] ?? 0),
  w: CARD_W,
  h: HEIGHTS[i] ?? 1.8,
  tilt: (TILT[i] ?? 0) * DEG,
  win: winFor(i),
  quoteColor: PALETTE.ink,
  quoteSize: i === 2 || i === 6 ? 0.2 : 0.24, // the two long quotes read smaller
}));
CARDS.push({
  quote: STARS,
  who: testimonials.starsWho,
  cx: COLS[2] ?? 0,
  cy: (ROWS[2] ?? 0) + (JIT[8] ?? 0),
  w: CARD_W,
  h: HEIGHTS[8] ?? 1.7,
  tilt: (TILT[8] ?? 0) * DEG,
  win: winFor(8),
  quoteColor: STAR_GOLD,
  quoteSize: 0.34,
});

// star scribbles + quote curls + thumbs-up, boiling beside the wall, staggered.
const DECOS: { name: string; pos: [number, number, number]; rot: [number, number, number]; scale: number; win: [number, number] }[] = [
  { name: "deco-testi-1", pos: [-7.6, 4.7, 0.2], rot: [0, 0, 0], scale: 0.045, win: [0.78, 0.812] },
  { name: "deco-testi-2", pos: [7.6, 4.7, 0.2], rot: [0, 0, Math.PI], scale: 0.045, win: [0.786, 0.818] },
  { name: "deco-testi-3", pos: [-7.7, 0.6, 0.2], rot: [0, 0, 0.1], scale: 0.05, win: [0.8, 0.83] },
  { name: "deco-testi-4", pos: [7.7, -1.4, 0.2], rot: [0, 0, -0.21], scale: 0.045, win: [0.82, 0.85] },
  { name: "deco-testi-5", pos: [-7.6, -3.6, 0.2], rot: [0, 0, 0.05], scale: 0.045, win: [0.83, 0.86] },
];

// Merged border batch: each card's rectangle, corners rotated by its tilt and
// translated to its centre, pushed as 4 edges (8 points) into ONE segments Line.
function useCardBorders(): [number, number, number][] {
  return useMemo(() => {
    const pts: [number, number, number][] = [];
    for (const c of CARDS) {
      const hw = c.w / 2;
      const hh = c.h / 2;
      const cos = Math.cos(c.tilt);
      const sin = Math.sin(c.tilt);
      const corner = (sx: number, sy: number): [number, number, number] => {
        const x = sx * hw;
        const y = sy * hh;
        return [c.cx + x * cos - y * sin, c.cy + x * sin + y * cos, 0.03];
      };
      const tl = corner(-1, 1);
      const tr = corner(1, 1);
      const br = corner(1, -1);
      const bl = corner(-1, -1);
      pts.push(tl, tr, tr, br, br, bl, bl, tl);
    }
    return pts;
  }, []);
}

export default function Testimonials() {
  const borders = useCardBorders();
  return (
    <group position={[26, 17, 0]}>
      {/* the pinboard page the notes sit on */}
      <mesh position={[0, 0, -0.5]}>
        <planeGeometry args={[26, 15]} />
        <meshBasicMaterial color={PALETTE.paper} side={DoubleSide} />
      </mesh>

      {/* crown: heading top (largest), then the sub-label */}
      <RevealText position={[0, 5.4, 0.1]} font={FONT.scrawl} characters={SCRAWL_CHARS} fontSize={0.66} maxWidth={14} color={PALETTE.ink} win={[0.77, 0.8]}>
        {testimonials.heading}
      </RevealText>
      <RevealText position={[0, 4.5, 0.1]} font={FONT.scrawl} characters={SCRAWL_CHARS} fontSize={0.28} maxWidth={12} color="#6c634d" win={[0.782, 0.808]}>
        {testimonials.headingSmall}
      </RevealText>

      {/* every card frame in one merged Line2 batch (one draw call) */}
      <Line points={borders} color={PALETTE.ink} segments worldUnits linewidth={0.028} />

      {/* the 9 notes, staggered reveal */}
      {CARDS.map((c, i) => (
        <Card key={i} {...c} />
      ))}

      {/* star scribbles / curls / thumbs boiling beside the wall */}
      {DECOS.map((d) => (
        <InkStrokes key={d.name} name={d.name} position={d.pos} rotation={d.rot} scale={d.scale} drawOn={{ t0: d.win[0], t1: d.win[1] }} />
      ))}

      {/* featured line: last, smallest, axis bottom */}
      <RevealText position={[0, -5.4, 0.1]} font={FONT.mono} characters={MONO_CHARS} fontSize={0.24} maxWidth={16} color="#6f6650" win={[0.85, 0.875]}>
        {FEATURED}
      </RevealText>
    </group>
  );
}
