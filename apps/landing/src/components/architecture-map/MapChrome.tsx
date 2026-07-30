import { Fragment } from 'react';

import { KBD_HELP_ROWS, LEGEND_ROWS } from '@/data/architecture-map';

// Differ only in id + fill — every other <marker> attribute is identical.
const EDGE_MARKERS = [
  { id: 'ar-critical', fill: '#ff3860' },
  { id: 'ar-api', fill: '#ff7a45' },
  { id: 'ar-db', fill: '#ffb86b' },
  { id: 'ar-mount', fill: '#4ea1ff' },
  { id: 'ar-normal', fill: '#3a3a3a' },
] as const;

// Arrowhead markers referenced by Layers.tsx's edge paths (url(#ar-<kind>)) —
// rendered inside <svg>, alongside <g id="vp">.
export function MapDefs() {
  return (
    <defs>
      {EDGE_MARKERS.map((m) => (
        <marker
          key={m.id}
          id={m.id}
          markerWidth={9}
          markerHeight={9}
          refX={7.5}
          refY={3}
          orient="auto"
        >
          <path d="M0,0 L7,3 L0,6 Z" fill={m.fill} />
        </marker>
      ))}
    </defs>
  );
}

export function MapControls() {
  return (
    <div className="controls">
      <button id="zin" type="button" title="Zoom in" aria-label="Zoom in">
        +
      </button>
      <button id="zout" type="button" title="Zoom out" aria-label="Zoom out">
        −
      </button>
      <button
        id="fit"
        type="button"
        title="Fit to screen"
        aria-label="Fit to screen"
        style={{ fontSize: '11px' }}
      >
        Fit
      </button>
      <button
        id="help-btn"
        type="button"
        title="Keyboard shortcuts (?)"
        aria-label="Keyboard shortcuts"
        aria-expanded={false}
      >
        ?
      </button>
    </div>
  );
}

export function MapLegend() {
  return (
    <div className="legend" id="legend">
      {LEGEND_ROWS.map((row, i) => (
        <div className="row" key={i}>
          {row.swatches.map((s, j) => (
            <Fragment key={j}>
              <span className={s.className} /> {s.label}
              {j < row.swatches.length - 1 ? '  ' : null}
            </Fragment>
          ))}
        </div>
      ))}
    </div>
  );
}

export function KbdHelpDialog() {
  return (
    <div id="kbd-help" hidden role="dialog" aria-modal="true" aria-labelledby="kbd-help-title">
      <div className="kbd-title">
        <span id="kbd-help-title">Keyboard shortcuts</span>
        <button id="kbd-help-close" type="button" aria-label="Close keyboard shortcuts">
          ✕
        </button>
      </div>
      <ul>
        {KBD_HELP_ROWS.map((row, i) => (
          <li key={i}>
            {row.map((part, j) => ('kbd' in part ? <kbd key={j}>{part.kbd}</kbd> : part.text))}
          </li>
        ))}
      </ul>
    </div>
  );
}
