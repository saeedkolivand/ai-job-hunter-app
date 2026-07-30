import type { Panel } from '@/components/architecture-map/Sidebar';
import { edges, nodes } from '@/data/architecture-map';

import { buildAdjacency, passEdgeFilter, passNodeFilter } from './filter';
import { type OnFn, ZOOM_FACTOR } from './geometry';
import { attachKeyboardShortcuts } from './keyboard';
import { attachViewport } from './viewport';

export interface AttachMapOptions {
  onPanel: (panel: Panel) => void;
}

// Interaction engine — imperative, ported from the original vanilla script.
// The static SVG structure renders declaratively (Layers.tsx); this wires
// pan/zoom/drag/keyboard/focus/a11y directly onto the prerendered DOM (hover/
// adjacency dimming toggles classes directly — only the low-frequency side
// panel goes through React state via opts.onPanel). Strict-mode-safe: every
// listener is torn down in the returned cleanup, so a double-invoke
// re-attaches cleanly.
export function attachMap(root: HTMLElement, opts: AttachMapOptions): () => void {
  const stage = root.querySelector<HTMLDivElement>('#stage');
  const svg = root.querySelector<SVGSVGElement>('#svg');
  const vp = root.querySelector<SVGGElement>('#vp');
  if (!stage || !svg || !vp) return () => {};

  const cleanups: Array<() => void> = [];
  const on: OnFn = (target, type, handler, listenerOpts) => {
    target.addEventListener(type, handler, listenerOpts);
    cleanups.push(() => target.removeEventListener(type, handler, listenerOpts));
  };

  // Element lookups (built once from the prerendered DOM) — one pass over all
  // .node groups keyed by data-id, so we never interpolate an id into a CSS
  // selector (robust to ids that would need escaping).
  const nodeEls = new Map<string, SVGGElement>();
  root.querySelectorAll<SVGGElement>('.node').forEach((g) => {
    const id = g.dataset.id;
    if (id) nodeEls.set(id, g);
  });
  const edgeEls: Array<{
    e: (typeof edges)[number];
    path: SVGPathElement;
    lbl: SVGTextElement | null;
  }> = [];
  edges.forEach((e, i) => {
    const path = root.querySelector<SVGPathElement>(`[data-edge="${i}"]`);
    if (!path) return;
    const lbl = root.querySelector<SVGTextElement>(`[data-edge-lbl="${i}"]`);
    edgeEls.push({ e, path, lbl });
  });

  const adj = buildAdjacency(nodes, edges);

  // view state
  let filter = 'overview';
  let pinned: string | null = null;
  let hover: string | null = null;
  const focusId = (): string | null => hover ?? pinned;

  function applyView() {
    const sel = focusId();
    const nb = sel ? new Set<string>([...(adj.get(sel) ?? []), sel]) : null;
    for (const n of nodes) {
      const g = nodeEls.get(n.id);
      if (!g) continue;
      const shown = nb ? nb.has(n.id) : passNodeFilter(n, filter);
      g.classList.toggle('dim', !shown);
    }
    for (const { e, path, lbl } of edgeEls) {
      const show = sel ? e.from === sel || e.to === sel : passEdgeFilter(e, filter);
      path.classList.toggle('hide', !show);
      if (lbl) lbl.classList.toggle('hide', !show);
    }
  }

  // Sidebar is React state; renderSidebar mirrors the original signature.
  const renderSidebar = (id: string | null) =>
    opts.onPanel(id ? { kind: 'node', id } : { kind: 'default' });

  // Screen-reader announcements — scoped to a tiny status line so ONLY
  // deliberate actions (pin/unpin, filter, clear) are spoken. The sidebar
  // itself is no longer a live region, so hover/focus sweeps stay silent.
  const statusEl = root.querySelector<HTMLElement>('#a11y-status');
  const announce = (msg: string) => {
    if (statusEl) statusEl.textContent = msg;
  };

  const viewport = attachViewport(stage, vp, on);

  const clearSelection = () => {
    const hadSelection = pinned !== null;
    pinned = null;
    hover = null;
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    renderSidebar(null);
    applyView();
    if (hadSelection) announce('Selection cleared');
  };

  // ── node interactions ───────────────────────────────────────────────────────
  for (const n of nodes) {
    const g = nodeEls.get(n.id);
    if (!g) continue;
    on(g, 'mouseenter', () => {
      hover = n.id;
      applyView();
      renderSidebar(focusId());
    });
    on(g, 'mouseleave', () => {
      hover = null;
      applyView();
      renderSidebar(focusId());
    });
    on(g, 'click', (ev) => {
      ev.stopPropagation();
      pinned = pinned === n.id ? null : n.id;
      hover = null;
      applyView();
      renderSidebar(pinned);
      announce(pinned ? `${n.label} selected` : 'Selection cleared');
    });
    on(g, 'focus', () => {
      hover = n.id;
      applyView();
      renderSidebar(focusId());
      viewport.ensureVisible(n);
    });
    on(g, 'blur', () => {
      hover = null;
      applyView();
      renderSidebar(focusId());
    });
    on(g, 'keydown', (ev) => {
      const ke = ev as KeyboardEvent;
      if (ke.key === 'Enter' || ke.key === ' ') {
        ke.preventDefault();
        pinned = pinned === n.id ? null : n.id;
        applyView();
        renderSidebar(pinned);
        announce(pinned ? `${n.label} selected` : 'Selection cleared');
      }
    });
  }

  // ── filter chips ────────────────────────────────────────────────────────────
  const chipBtns = Array.from(root.querySelectorAll<HTMLButtonElement>('#chips button'));
  for (const b of chipBtns) {
    on(b, 'click', () => {
      const id = b.dataset.f ?? 'overview';
      filter = id;
      pinned = null;
      hover = null;
      for (const c of chipBtns) {
        const active = c.dataset.f === id;
        c.classList.toggle('active', active);
        c.setAttribute('aria-pressed', active ? 'true' : 'false');
      }
      renderSidebar(null);
      applyView();
      announce(`Filter: ${b.textContent?.trim() ?? id}`);
    });
  }

  // ── zoom controls ───────────────────────────────────────────────────────────
  const zin = root.querySelector<HTMLButtonElement>('#zin');
  const zout = root.querySelector<HTMLButtonElement>('#zout');
  const fitBtn = root.querySelector<HTMLButtonElement>('#fit');
  if (zin) on(zin, 'click', () => viewport.zoomAt(...viewport.stageCenter(), ZOOM_FACTOR));
  if (zout) on(zout, 'click', () => viewport.zoomAt(...viewport.stageCenter(), 1 / ZOOM_FACTOR));
  if (fitBtn) on(fitBtn, 'click', () => viewport.fit());

  on(stage, 'click', (ev) => {
    // A pan that ends on the background must not clear the selection.
    if (viewport.consumeDragMoved()) return;
    const target = ev.target as Element;
    if (target === stage || target === svg || target.id === 'vp') {
      const hadSelection = pinned !== null;
      pinned = null;
      hover = null;
      renderSidebar(null);
      applyView();
      if (hadSelection) announce('Selection cleared');
    }
  });

  attachKeyboardShortcuts(root, on, { viewport, onEscape: clearSelection });

  // ── hand scaling off from the SVG viewBox to our pixel-space transform ───────
  // The static markup keeps viewBox + preserveAspectRatio so the prerendered map
  // fits the stage *before* hydration (the no-JS baseline). But once the engine
  // takes over we own the mapping via the #vp transform, so the viewBox has to
  // go: otherwise the browser maps 3060→stage AND fit() scales again on top of
  // it (the double-scaling that shrank the map to a corner sliver). With the
  // viewBox removed the svg (CSS width/height 100%) is 1:1 with stage pixels, so
  // fit()/zoomAt()'s pixel-space math is literally correct.
  const savedViewBox = svg.getAttribute('viewBox');
  const savedPreserveAspectRatio = svg.getAttribute('preserveAspectRatio');
  svg.removeAttribute('viewBox');
  svg.removeAttribute('preserveAspectRatio');

  // ── boot ────────────────────────────────────────────────────────────────────
  renderSidebar(null);
  applyView();
  viewport.fit();

  return () => {
    for (const fn of cleanups) fn();
    // Restore the prerendered attributes so strict-mode's double-invoke (and any
    // unmount) leaves the DOM in the viewBox-driven no-JS state; the next effect
    // run re-captures and removes them again.
    if (savedViewBox !== null) svg.setAttribute('viewBox', savedViewBox);
    else svg.removeAttribute('viewBox');
    if (savedPreserveAspectRatio !== null)
      svg.setAttribute('preserveAspectRatio', savedPreserveAspectRatio);
    else svg.removeAttribute('preserveAspectRatio');
  };
}
