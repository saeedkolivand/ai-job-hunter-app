import { memo } from 'react';

import { clusters, COLORS, edges, FIXES, KNOWN_BUGS, nodes } from '@/data/architecture-map';
import { edgePath } from '@/lib/architecture-map/geometry';

import { nodeAria } from './Sidebar';

const byId = new Map(nodes.map((n) => [n.id, n]));

// ── static SVG structure (prerendered; the interaction engine mutates it live) ─
// These three take no props and read only module-level data, so they are wrapped
// in memo(): the sidebar's per-hover setState re-renders ArchitectureMap, but the
// ~280 static SVG elements below never rebuild (empty props always compare equal).
export const Clusters = memo(function Clusters() {
  return (
    <>
      {clusters.map((c) => {
        const col = COLORS[c.color] ?? '#888';
        return (
          <g key={c.id}>
            <rect
              className="clusterBox"
              x={c.x}
              y={c.y}
              width={c.w}
              height={c.h}
              rx={14}
              fill={col}
              stroke={col}
              fillOpacity={0.05}
              strokeOpacity={0.3}
            />
            <text className="clusterLabel" x={c.x + 14} y={c.y + 28} fill={col} fillOpacity={0.85}>
              {c.label}
            </text>
          </g>
        );
      })}
    </>
  );
});

export const Edges = memo(function Edges() {
  return (
    <>
      {edges.map((e, i) => {
        const a = byId.get(e.from);
        const b = byId.get(e.to);
        if (!a || !b) return null;
        const p = edgePath(a, b);
        return (
          <g key={`${e.from}->${e.to}-${i}`}>
            <path
              data-edge={i}
              className={`edge kind-${e.kind}`}
              d={p.d}
              markerEnd={`url(#ar-${e.kind})`}
            />
            {e.label ? (
              <text
                data-edge-lbl={i}
                className={`edgeLabel${e.kind === 'critical' ? ' crit' : ''}`}
                x={p.mx}
                y={p.my - 3 + ((i % 3) - 1) * 6}
                textAnchor="middle"
              >
                {e.label}
              </text>
            ) : null}
          </g>
        );
      })}
    </>
  );
});

export const Nodes = memo(function Nodes() {
  return (
    <>
      {nodes.map((n) => {
        const col = COLORS[n.color] ?? '#888';
        const fixes = FIXES[n.id];
        const bugs = KNOWN_BUGS[n.id];
        return (
          <g
            key={n.id}
            className={`node${n.critical ? ' crit' : ''}`}
            data-id={n.id}
            tabIndex={0}
            role="button"
            aria-label={nodeAria(n)}
          >
            <rect className="box" x={n.x} y={n.y} width={n.w} height={n.h} rx={8} stroke={col} />
            <text className="lbl" x={n.x + 12} y={n.y + (n.sub ? n.h / 2 - 1 : n.h / 2 + 4)}>
              {n.label}
            </text>
            {n.sub ? (
              <text className="sub" x={n.x + 12} y={n.y + n.h / 2 + 13}>
                {n.sub}
              </text>
            ) : null}
            {fixes ? (
              <g className="badge">
                <circle
                  cx={n.x + n.w - 10}
                  cy={n.y + 10}
                  r={8}
                  fill="#5fd96b"
                  stroke="#0c0c0c"
                  strokeWidth={1}
                />
                <text x={n.x + n.w - 10} y={n.y + 13} textAnchor="middle">
                  {fixes.length}
                </text>
              </g>
            ) : null}
            {bugs ? (
              <g className="badge">
                <circle
                  cx={n.x + n.w - (fixes ? 28 : 10)}
                  cy={n.y + 10}
                  r={8}
                  fill="#ff4d6a"
                  stroke="#0c0c0c"
                  strokeWidth={1}
                />
                <text
                  x={n.x + n.w - (fixes ? 28 : 10)}
                  y={n.y + 13}
                  textAnchor="middle"
                  fill="#fff"
                >
                  {bugs.length}
                </text>
              </g>
            ) : null}
          </g>
        );
      })}
    </>
  );
});
