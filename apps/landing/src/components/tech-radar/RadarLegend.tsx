import { RINGS } from '@/data/tech-radar';

import { RadarGlyph } from './RadarGlyph';

// Sighted-user key for the ring shapes drawn in <RadarSvg> — decorative
// (aria-hidden): the same information is stated in words on every entry in
// <RadarList>, so a screen reader never depends on this key existing.
export function RadarLegend() {
  return (
    <ul className="tr-legend" aria-hidden="true">
      {RINGS.map((ring) => (
        <li key={ring.id} className={`tr-legend__item tr-legend__item--${ring.id}`}>
          <svg className="tr-legend__icon" viewBox="0 0 28 28" focusable="false">
            <g transform="translate(14 14)">
              <RadarGlyph ring={ring.id} size={15} className="tr-legend__mark" />
            </g>
          </svg>
          {ring.label}
        </li>
      ))}
    </ul>
  );
}
