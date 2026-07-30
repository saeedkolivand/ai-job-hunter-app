'use client';

import { useEffect, useRef, useState } from 'react';

import { CHIPS } from '@/data/architecture-map';
import { CANVAS } from '@/lib/architecture-map/geometry';
import { attachMap } from '@/lib/architecture-map/interactions';

import { Clusters, Edges, Nodes } from './Layers';
import { KbdHelpDialog, MapControls, MapDefs, MapLegend } from './MapChrome';
import { type Panel, Sidebar } from './Sidebar';

// Fired once from the effect below; a module-level flag keeps React
// strict-mode's double-invoke (and any remount) from logging the banner twice.
let easterEggLogged = false;

// ── console easter egg — devtools hello, in the wiring's voice (once) ────────
// Byte-faithful port of the original static page's console banner.
function logArchitectureMapBanner() {
  if (easterEggLogged) return;
  easterEggLogged = true;
  try {
    const head = 'color:#0f0f0f;background:#f5b942;font:700 18px/1.4 monospace;padding:6px 12px';
    const blue = 'color:#4ea1ff;font:13px/1.7 monospace';
    const soft = 'color:#8a8a8a;font:13px/1.7 monospace';
    console.log("%c ◇ you're reading the wiring ", head);
    console.log(
      '%cevery node maps to a real file. trace it yourself 👉 https://github.com/saeedkolivand/ai-job-hunter-app',
      blue
    );
    console.log(
      '%c(a drift checker fails CI if this diagram ever lies. it has caught me twice.)',
      soft
    );
  } catch {
    // devtools console banner is best-effort — never break hydration over it
  }
}

export function ArchitectureMap() {
  const [panel, setPanel] = useState<Panel>({ kind: 'default' });
  const rootRef = useRef<HTMLDivElement>(null);

  // Interaction engine — imperative, ported from the original vanilla script
  // (see lib/architecture-map/interactions.ts). Static structure renders
  // declaratively above; pan/zoom/hover/filter mutate the prerendered DOM
  // directly (no React state per event → 60fps). Only the low-frequency side
  // panel is React state (setPanel). Strict-mode-safe: attachMap's cleanup
  // tears down every listener, so a double-invoke re-attaches cleanly.
  useEffect(() => {
    logArchitectureMapBanner();
    const root = rootRef.current;
    if (!root) return;
    return attachMap(root, { onPanel: setPanel });
  }, []);

  return (
    <div className="arch-map" ref={rootRef}>
      <header>
        <h1>
          AI Job Hunter
          <span className="meta">
            {' '}
            — interactive architecture map · local-first Tauri 2 monorepo
          </span>
        </h1>
        <div className="chips" id="chips">
          {CHIPS.map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={`chip${id === 'overview' ? ' active' : ''}${id === 'bugs' ? ' bugchip' : ''}`}
              data-f={id}
              aria-pressed={id === 'overview'}
            >
              {label}
            </button>
          ))}
        </div>
      </header>
      <main>
        <div id="stage">
          <svg id="svg" viewBox={`0 0 ${CANVAS.w} ${CANVAS.h}`} preserveAspectRatio="xMidYMid meet">
            <MapDefs />
            <g id="vp">
              <g id="gClusters">
                <Clusters />
              </g>
              <g id="gEdges">
                <Edges />
              </g>
              <g id="gNodes">
                <Nodes />
              </g>
            </g>
          </svg>
          <MapControls />
          <MapLegend />
          <KbdHelpDialog />
        </div>
        <aside id="side">
          <Sidebar panel={panel} />
        </aside>
      </main>
      {/* Scoped live region — announces only deliberate actions (pin/unpin,
          filter, clear), so screen readers don't narrate every hover/focus. */}
      <div
        id="a11y-status"
        className="sr-only"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      />
    </div>
  );
}
