import type { RadarQuadrant, RadarRing, TechRadarEntry } from '@/data/tech-radar';

// Pure polar→cartesian layout for the /tech-radar SVG. No pan/zoom, no DOM
// measurement (unlike architecture-map's interactions.ts) — the whole radar
// fits in one static viewBox, so this is plain math, safe to call from a
// server component.

export const VIEW_SIZE = 600;
const CENTER = VIEW_SIZE / 2;

// Ring bands (innermost = Adopt, most settled) — [innerRadius, outerRadius].
export const RING_BANDS: Readonly<Record<RadarRing, readonly [number, number]>> = {
  adopt: [36, 118],
  trial: [118, 198],
  assess: [198, 278],
  hold: [278, 352],
};

// Quadrant wedges, in degrees clockwise from 12 o'clock.
const QUADRANT_START: Readonly<Record<RadarQuadrant, number>> = {
  'renderer-ui': 0,
  'backend-data': 90,
  'documents-export': 180,
  'build-ship-trust': 270,
};

export interface BlipPosition {
  x: number;
  y: number;
}

// Every blip's SVG <a> carries an invisible hit-circle this wide (see
// RadarSvg.tsx's .tr-blip__hit) — the ACTUAL clickable/focusable target,
// bigger than the drawn marker. Owned here, not RadarSvg.tsx, because the
// layout below is tuned against it (see minPairwiseDistance and its test).
export const HIT_RADIUS = 14;

// tech-radar.css hides <RadarSvg> below this width (`@media (max-width:
// 679px)`) and caps `.tr-svg`'s CSS width at exactly this value — so
// whenever the canvas IS shown, its rendered width is >= this number,
// giving a worst-case viewBox-unit -> CSS-px scale of
// SVG_MIN_SHOWN_WIDTH_PX / VIEW_SIZE. Below this width the full data is
// still reachable via the always-rendered <RadarList>, so nothing is lost.
// KEEP IN SYNC with tech-radar.css by hand (CSS can't import a TS
// constant) — the geometry test asserts what this buys us in real px.
export const SVG_MIN_SHOWN_WIDTH_PX = 680;

// 14deg, not a smaller value: the FIRST failure numeric search turned up
// here wasn't a crowded single cell, it was two ADJACENT quadrants at the
// SAME ring (e.g. renderer-ui/adopt's last blip and backend-data/adopt's
// first) landing close enough across the quadrant boundary to collide —
// same-cell spacing alone doesn't prevent that; only the padding does. Both
// this and RADIUS_FRACTIONS below were chosen together, numerically, against
// this file's REAL data (every quadrant+ring cell, both boundary and
// within-cell pairs) — not tuned in isolation. See geometry.test.ts.
const WEDGE_PAD_DEG = 14;
const WEDGE_SPAN_DEG = 90 - WEDGE_PAD_DEG * 2;
// Four radius "lanes" a crowded ring+quadrant cell's blips cycle through, so
// they don't all draw on one arc.
const RADIUS_FRACTIONS = [0.18, 0.45, 0.71, 0.98];

/**
 * Deterministic (id-ordered, no randomness) blip position for every entry,
 * keyed by entry id. Entries sharing a (quadrant, ring) cell are spread
 * evenly across that cell's angular wedge.
 */
export function layoutBlips(entries: readonly TechRadarEntry[]): ReadonlyMap<string, BlipPosition> {
  const cells = new Map<string, TechRadarEntry[]>();
  for (const entry of entries) {
    const key = `${entry.quadrant}:${entry.ring}`;
    const group = cells.get(key);
    if (group) group.push(entry);
    else cells.set(key, [entry]);
  }

  const positions = new Map<string, BlipPosition>();
  for (const [key, group] of cells) {
    const [quadrant, ring] = key.split(':') as [RadarQuadrant, RadarRing];
    const [innerR, outerR] = RING_BANDS[ring];
    const start = QUADRANT_START[quadrant];
    group.forEach((entry, i) => {
      const t = group.length === 1 ? 0.5 : i / (group.length - 1);
      const angleDeg = start + WEDGE_PAD_DEG + t * WEDGE_SPAN_DEG;
      const radiusFraction = RADIUS_FRACTIONS[i % RADIUS_FRACTIONS.length] ?? 0.5;
      const radius = innerR + (outerR - innerR) * radiusFraction;
      const angleRad = (angleDeg * Math.PI) / 180;
      positions.set(entry.id, {
        x: CENTER + radius * Math.sin(angleRad),
        y: CENTER - radius * Math.cos(angleRad),
      });
    });
  }
  return positions;
}

/** Smallest center-to-center distance between any two positions, in viewBox units. */
export function minPairwiseDistance(positions: ReadonlyMap<string, BlipPosition>): number {
  const pts = [...positions.values()];
  let min = Infinity;
  for (let i = 0; i < pts.length; i++) {
    for (let j = i + 1; j < pts.length; j++) {
      const a = pts[i];
      const b = pts[j];
      if (!a || !b) continue;
      const d = Math.hypot(a.x - b.x, a.y - b.y);
      if (d < min) min = d;
    }
  }
  return min;
}
