import { QUADRANTS, RADAR, RINGS } from '@/data/tech-radar';
import { HIT_RADIUS, layoutBlips, RING_BANDS, VIEW_SIZE } from '@/lib/tech-radar/geometry';

import { RadarGlyph } from './RadarGlyph';

// The visual radar. Every blip is an SVG <a> — natively keyboard-focusable,
// no client JS required — jumping to its full write-up in <RadarList>. Ring
// is encoded by BOTH shape (RadarGlyph) and color, never color alone. Hidden
// below the breakpoint in tech-radar.css, where <RadarList> below carries
// the whole radar as a real list instead — see that file for why a circular
// layout doesn't survive a phone-width viewport.
const CENTER = VIEW_SIZE / 2;

const SHAPE_SIZE = 15; // ~half-width of each blip's visible marker
// HIT_RADIUS (from geometry.ts, shared with the layout tuning) makes the
// actual focusable/hit target bigger than the drawn marker. Two guarantees,
// both verified by geometry.test.ts against SVG_MIN_SHOWN_WIDTH_PX (the
// narrowest width tech-radar.css ever shows this canvas at):
//  1. Every blip's own hit-circle independently clears the WCAG 2.5.8
//     24x24px floor — true regardless of density (it only depends on
//     HIT_RADIUS and the render scale, never on neighboring blips).
//  2. In the densest ring+quadrant cell this data has today, adjacent
//     blips' hit-circles stay far enough apart to stay individually
//     tappable too — that ONE is density-dependent (RADIUS_FRACTIONS in
//     geometry.ts is tuned against it), so it degrades gracefully rather
//     than silently if a cell ever gets crowded enough to break it.

export function RadarSvg() {
  const positions = layoutBlips(RADAR);
  const quadrantLabel = new Map(QUADRANTS.map((q) => [q.id, q.label]));
  const ringLabel = new Map(RINGS.map((r) => [r.id, r.label]));

  return (
    <svg
      className="tr-svg"
      viewBox={`0 0 ${VIEW_SIZE} ${VIEW_SIZE}`}
      role="group"
      aria-label="Technology radar, visual overview. Every item is also listed as text below with its full write-up."
    >
      <g aria-hidden="true">
        {/* Ring bands (outer to inner, so inner paints on top). */}
        {RINGS.slice()
          .reverse()
          .map((ring) => (
            <circle
              key={ring.id}
              className={`tr-ring tr-ring--${ring.id}`}
              cx={CENTER}
              cy={CENTER}
              r={RING_BANDS[ring.id][1]}
            />
          ))}
        {/* Quadrant divider cross. */}
        <line className="tr-divider" x1={CENTER} y1={0} x2={CENTER} y2={VIEW_SIZE} />
        <line className="tr-divider" x1={0} y1={CENTER} x2={VIEW_SIZE} y2={CENTER} />
        {/* Ring labels, one per band, on the vertical divider. */}
        {RINGS.map((ring) => (
          <text
            key={ring.id}
            className="tr-ring-label"
            x={CENTER + 4}
            y={CENTER - (RING_BANDS[ring.id][0] + RING_BANDS[ring.id][1]) / 2}
          >
            {ring.label}
          </text>
        ))}
        {/* Quadrant labels, one per corner. */}
        <text className="tr-quadrant-label" x={VIEW_SIZE - 8} y={16} textAnchor="end">
          {quadrantLabel.get('renderer-ui')}
        </text>
        <text className="tr-quadrant-label" x={VIEW_SIZE - 8} y={VIEW_SIZE - 8} textAnchor="end">
          {quadrantLabel.get('backend-data')}
        </text>
        <text className="tr-quadrant-label" x={8} y={VIEW_SIZE - 8}>
          {quadrantLabel.get('documents-export')}
        </text>
        <text className="tr-quadrant-label" x={8} y={16}>
          {quadrantLabel.get('build-ship-trust')}
        </text>
      </g>

      {RADAR.map((item, i) => {
        const pos = positions.get(item.id);
        if (!pos) return null;
        const label = `${item.name} — ${ringLabel.get(item.ring)} — ${quadrantLabel.get(item.quadrant)}`;
        return (
          <a
            key={item.id}
            className={`tr-blip tr-blip--${item.ring}`}
            href={`#tr-entry-${item.id}`}
            aria-label={`${label}. Jump to full entry below.`}
          >
            <title>{label}</title>
            <g transform={`translate(${pos.x} ${pos.y})`}>
              <circle className="tr-blip__hit" r={HIT_RADIUS} />
              <RadarGlyph ring={item.ring} size={SHAPE_SIZE} className="tr-blip__mark" />
              <text className="tr-blip__num" textAnchor="middle" dy="0.32em">
                {i + 1}
              </text>
            </g>
          </a>
        );
      })}
    </svg>
  );
}
