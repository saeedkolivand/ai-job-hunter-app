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

const WEDGE_PAD_DEG = 9;
const WEDGE_SPAN_DEG = 90 - WEDGE_PAD_DEG * 2;
// Alternates each blip's radius within its ring band (inner/mid/outer third)
// so that a crowded ring+quadrant cell doesn't draw every dot on one arc.
const RADIUS_FRACTIONS = [0.28, 0.72, 0.5];

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
