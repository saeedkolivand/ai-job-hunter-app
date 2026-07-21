'use client';

import { memo, type ReactNode, useEffect, useRef, useState } from 'react';

import {
  CHIPS,
  clusters,
  COLORS,
  edges,
  FINDINGS,
  FIXES,
  KNOWN_BUGS,
  type MapNode,
  nodes,
} from '@/data/architecture-map';

// Node lookup — the map's edges/sidebar resolve nodes by id, exactly like the
// original `byId` in the passthrough dashboard.
const byId = new Map(nodes.map((n) => [n.id, n]));

// Fixed authoring canvas — the single source of truth for both the static
// <svg viewBox> (no-JS baseline) and fit()'s pixel-space scale math, so the two
// can never drift apart.
const CANVAS = { w: 3060, h: 1330 };

// ── deterministic geometry (ported verbatim from the original render engine) ──
// Edge paths are a pure function of the two nodes' fixed coordinates, so they are
// computed once here and rendered as static <path> markup (prerendered at build).
function edgePath(a: MapNode, b: MapNode): { d: string; mx: number; my: number } {
  const x1 = a.x + a.w;
  const y1 = a.y + a.h / 2;
  const x2 = b.x;
  const y2 = b.y + b.h / 2;
  // If b is to the left (a return edge), exit/enter on the sensible sides.
  let sx = x1;
  const sy = y1;
  let ex = x2;
  const ey = y2;
  if (b.x < a.x) {
    sx = a.x;
    ex = b.x + b.w;
  }
  const dx = Math.max(40, Math.abs(ex - sx) * 0.45);
  const c1x = sx + (ex >= sx ? dx : -dx);
  const c2x = ex - (ex >= sx ? dx : -dx);
  return {
    d: `M ${sx} ${sy} C ${c1x} ${sy}, ${c2x} ${ey}, ${ex} ${ey}`,
    mx: (sx + ex) / 2,
    my: (sy + ey) / 2,
  };
}

function nodeAria(n: MapNode): string {
  return `${n.label}${n.sub ? ' — ' + n.sub : ''}${n.role ? '. ' + n.role : ''}`;
}

// Render verbatim findings prose, turning its <b>…</b> emphasis into real React
// elements — never innerHTML / dangerouslySetInnerHTML (ADR-0018 origin invariant).
function renderRich(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /<b>(.*?)<\/b>/g;
  let last = 0;
  let key = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) out.push(text.slice(last, m.index));
    out.push(<b key={key++}>{m[1]}</b>);
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

// ── sidebar (React state — low-frequency: changes on hover/click/clear) ───────
type Panel = { kind: 'default' } | { kind: 'node'; id: string };

function DefaultSidebar() {
  return (
    <>
      <h2>AI Job Hunter — architecture</h2>
      <div className="role">
        Local-first Tauri 2 desktop app. The renderer talks to a Rust core only through typed IPC
        contracts (ports &amp; adapters). Heavy work — scraping, document extraction, AI generation,
        embeddings — runs natively in-process.
      </div>
      <div className="plain">
        Click any box for its plain-English role, real file path, and what wires into and out of it.
        Use the chips up top to highlight a feature or a user-flow path. Drag to pan, scroll to
        zoom.
      </div>
      <div className="k">Notable findings from this map</div>
      <ul className="findings">
        {FINDINGS.map((f, i) => (
          <li key={i}>{renderRich(f)}</li>
        ))}
      </ul>
      <div className="k">Counts</div>
      <div style={{ color: 'var(--faint)' }}>
        {nodes.length} nodes · {edges.length} edges · {clusters.length} clusters · 24 boards · 8 AI
        providers
      </div>
      <div className="k">Critical paths</div>
      <div style={{ color: 'var(--faint)' }}>
        Red = <b>AI Generate</b> (default). Click <b>Autopilot</b> or <b>Scrape → Match</b> to light
        up the other two flows.
      </div>
    </>
  );
}

function NodeSidebar({ node: n }: { node: MapNode }) {
  const ins = edges.filter((e) => e.to === n.id);
  const outs = edges.filter((e) => e.from === n.id);
  const fixes = FIXES[n.id];
  const bugs = KNOWN_BUGS[n.id];
  return (
    <>
      <h2>{n.label}</h2>
      {n.sub ? (
        <div style={{ color: 'var(--muted)', fontSize: '11px', marginBottom: '6px' }}>{n.sub}</div>
      ) : null}
      {n.role ? <div className="role">{n.role}</div> : null}
      {n.plain ? <div className="plain">{n.plain}</div> : null}
      {n.path ? <div className="path">{n.path}</div> : null}
      {bugs ? (
        <>
          <div className="k">Known bugs</div>
          {bugs.map((b, i) => (
            <div className="bug" key={i}>
              <span className="sev">{b.sev}</span>
              {b.t}
              <div style={{ opacity: 0.7, fontSize: '10px', marginTop: '3px' }}>{b.ref}</div>
            </div>
          ))}
        </>
      ) : null}
      {fixes ? (
        <>
          <div className="k">Roadmap / fixes</div>
          {fixes.map((f, i) => (
            <div className="fix" key={i}>
              #{f.n} · {f.t}
            </div>
          ))}
        </>
      ) : null}
      {n.notes.length > 0 ? (
        <>
          <div className="k">Notes</div>
          <ul>
            {n.notes.map((x, i) => (
              <li key={i}>{x}</li>
            ))}
          </ul>
        </>
      ) : null}
      <div className="k">Wires in ({ins.length})</div>
      {ins.map((e, i) => (
        <div className="edgepair" key={i}>
          ← <b>{byId.get(e.from)?.label ?? e.from}</b> · {e.label ?? e.kind}
        </div>
      ))}
      <div className="k">Wires out ({outs.length})</div>
      {outs.map((e, i) => (
        <div className="edgepair" key={i}>
          → <b>{byId.get(e.to)?.label ?? e.to}</b> · {e.label ?? e.kind}
        </div>
      ))}
      <div className="k">Tags</div>
      <div>
        {n.tag.map((t, i) => (
          <span className="tag" key={i}>
            {t}
          </span>
        ))}
      </div>
    </>
  );
}

function Sidebar({ panel }: { panel: Panel }) {
  if (panel.kind === 'node') {
    const n = byId.get(panel.id);
    if (n) return <NodeSidebar node={n} />;
  }
  return <DefaultSidebar />;
}

// ── static SVG structure (prerendered; the interaction engine mutates it live) ─
// These three take no props and read only module-level data, so they are wrapped
// in memo(): the sidebar's per-hover setState re-renders ArchitectureMap, but the
// ~280 static SVG elements below never rebuild (empty props always compare equal).
const Clusters = memo(function Clusters() {
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

const Edges = memo(function Edges() {
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

const Nodes = memo(function Nodes() {
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

// Fired once from the interaction effect below; a module-level flag keeps React
// strict-mode's double-invoke (and any remount) from logging the banner twice.
let easterEggLogged = false;

export function ArchitectureMap() {
  const [panel, setPanel] = useState<Panel>({ kind: 'default' });
  const rootRef = useRef<HTMLDivElement>(null);

  // ── interaction engine — imperative, ported from the original vanilla script.
  // Static structure above renders declaratively; pan/zoom manipulate the SVG
  // group transform directly (no React state per wheel/pointer event → 60fps),
  // and hover/adjacency dimming toggles classes directly. Only the low-frequency
  // side panel is React state (setPanel). Strict-mode-safe: every listener is
  // torn down in the cleanup, so a double-invoke re-attaches cleanly.
  useEffect(() => {
    // ── console easter egg — devtools hello, in the wiring's voice (once) ────────
    // Byte-faithful port of the original static page's console banner. The
    // module-level flag guards against strict-mode's double-invoke so it logs once.
    if (!easterEggLogged) {
      easterEggLogged = true;
      try {
        const head =
          'color:#0f0f0f;background:#f5b942;font:700 18px/1.4 monospace;padding:6px 12px';
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

    const root = rootRef.current;
    if (!root) return;
    const stage = root.querySelector<HTMLDivElement>('#stage');
    const svg = root.querySelector<SVGSVGElement>('#svg');
    const vp = root.querySelector<SVGGElement>('#vp');
    if (!stage || !svg || !vp) return;

    const cleanups: Array<() => void> = [];
    const on = <T extends EventTarget>(
      target: T,
      type: string,
      handler: EventListenerOrEventListenerObject,
      opts?: AddEventListenerOptions
    ) => {
      target.addEventListener(type, handler, opts);
      cleanups.push(() => target.removeEventListener(type, handler, opts));
    };

    // element lookups (built once from the prerendered DOM) — one pass over all
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

    // adjacency for selection highlighting
    const adj = new Map<string, Set<string>>();
    for (const n of nodes) adj.set(n.id, new Set());
    for (const e of edges) {
      adj.get(e.from)?.add(e.to);
      adj.get(e.to)?.add(e.from);
    }

    // view state
    let filter = 'overview';
    let pinned: string | null = null;
    let hover: string | null = null;
    const focusId = (): string | null => hover ?? pinned;

    const hasBadge = (id: string) => Boolean(FIXES[id] ?? KNOWN_BUGS[id]);
    const passNodeFilter = (n: MapNode): boolean => {
      if (filter === 'all' || filter === 'overview') return true;
      if (filter === 'bugs') return hasBadge(n.id);
      return n.tag.includes(filter);
    };
    const passEdgeFilter = (e: (typeof edges)[number]): boolean => {
      if (filter === 'all') return true;
      if (filter === 'overview') return e.kind !== 'normal' || e.tag.includes('overview');
      if (filter === 'bugs') return false;
      return e.tag.includes(filter);
    };

    function applyView() {
      const sel = focusId();
      const nb = sel ? new Set<string>([...(adj.get(sel) ?? []), sel]) : null;
      for (const n of nodes) {
        const g = nodeEls.get(n.id);
        if (!g) continue;
        const shown = nb ? nb.has(n.id) : passNodeFilter(n);
        g.classList.toggle('dim', !shown);
      }
      for (const { e, path, lbl } of edgeEls) {
        const show = sel ? e.from === sel || e.to === sel : passEdgeFilter(e);
        path.classList.toggle('hide', !show);
        if (lbl) lbl.classList.toggle('hide', !show);
      }
    }

    // sidebar is React state; renderSidebar mirrors the original signature.
    const renderSidebar = (id: string | null) =>
      setPanel(id ? { kind: 'node', id } : { kind: 'default' });

    // Screen-reader announcements — scoped to a tiny status line so ONLY
    // deliberate actions (pin/unpin, filter, clear) are spoken. The sidebar
    // itself is no longer a live region, so hover/focus sweeps stay silent.
    const statusEl = root.querySelector<HTMLElement>('#a11y-status');
    const announce = (msg: string) => {
      if (statusEl) statusEl.textContent = msg;
    };

    // ── pan + zoom (imperative transform on the #vp group) ──────────────────────
    let tx = 0;
    let ty = 0;
    let scale = 1;
    const applyTransform = () =>
      vp.setAttribute('transform', `translate(${tx} ${ty}) scale(${scale})`);
    const fit = () => {
      const r = stage.getBoundingClientRect();
      scale = Math.min(r.width / CANVAS.w, r.height / CANVAS.h) * 0.98;
      tx = (r.width - CANVAS.w * scale) / 2;
      ty = (r.height - CANVAS.h * scale) / 2;
      applyTransform();
    };
    const screenToVB = (px: number, py: number) => {
      const r = stage.getBoundingClientRect();
      return { x: px - r.left, y: py - r.top };
    };
    const stageCenter = (): [number, number] => {
      const r = stage.getBoundingClientRect();
      return [r.width / 2, r.height / 2];
    };
    function zoomAt(cx: number, cy: number, f: number) {
      const next = Math.max(0.15, Math.min(4, scale * f));
      const k = next / scale;
      tx = cx - (cx - tx) * k;
      ty = cy - (cy - ty) * k;
      scale = next;
      applyTransform();
    }
    const ensureVisible = (n: MapNode) => {
      const r = stage.getBoundingClientRect();
      const m = 90;
      const cx = (n.x + n.w / 2) * scale + tx;
      const cy = (n.y + n.h / 2) * scale + ty;
      let dx = 0;
      let dy = 0;
      if (cx < m) dx = m - cx;
      else if (cx > r.width - m) dx = r.width - m - cx;
      if (cy < m) dy = m - cy;
      else if (cy > r.height - m) dy = r.height - m - cy;
      if (dx || dy) {
        tx += dx;
        ty += dy;
        applyTransform();
      }
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
        ensureVisible(n);
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
    if (zin) on(zin, 'click', () => zoomAt(...stageCenter(), 1.2));
    if (zout) on(zout, 'click', () => zoomAt(...stageCenter(), 1 / 1.2));
    if (fitBtn) on(fitBtn, 'click', () => fit());

    // ── wheel zoom + drag pan ───────────────────────────────────────────────────
    on(
      stage,
      'wheel',
      (ev) => {
        const we = ev as WheelEvent;
        we.preventDefault();
        const f = we.deltaY < 0 ? 1.12 : 1 / 1.12;
        const p = screenToVB(we.clientX, we.clientY);
        zoomAt(p.x, p.y, f);
      },
      { passive: false }
    );
    let dragging = false;
    let moved = false;
    let lx = 0;
    let ly = 0;
    on(stage, 'mousedown', (ev) => {
      const me = ev as MouseEvent;
      if (me.button !== 0) return;
      dragging = true;
      moved = false;
      lx = me.clientX;
      ly = me.clientY;
      stage.classList.add('panning');
    });
    on(window, 'mousemove', (ev) => {
      if (!dragging) return;
      const me = ev as MouseEvent;
      if (Math.abs(me.clientX - lx) + Math.abs(me.clientY - ly) > 3) moved = true;
      tx += me.clientX - lx;
      ty += me.clientY - ly;
      lx = me.clientX;
      ly = me.clientY;
      applyTransform();
    });
    on(window, 'mouseup', () => {
      dragging = false;
      stage.classList.remove('panning');
    });
    on(stage, 'click', (ev) => {
      // A pan that ends on the background must not clear the selection.
      if (moved) {
        moved = false;
        return;
      }
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

    // ── keyboard shortcuts + help overlay ───────────────────────────────────────
    const helpEl = root.querySelector<HTMLDivElement>('#kbd-help');
    const helpBtn = root.querySelector<HTMLButtonElement>('#help-btn');
    const helpClose = root.querySelector<HTMLButtonElement>('#kbd-help-close');

    const helpFocusables = (): HTMLElement[] =>
      helpEl
        ? Array.from(
            helpEl.querySelectorAll<HTMLElement>(
              'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
            )
          ).filter((el) => !el.hasAttribute('disabled'))
        : [];
    const openHelp = () => {
      if (!helpEl) return;
      helpEl.removeAttribute('hidden');
      helpBtn?.setAttribute('aria-expanded', 'true');
      helpFocusables()[0]?.focus();
    };
    const closeHelp = () => {
      if (!helpEl) return;
      helpEl.setAttribute('hidden', '');
      helpBtn?.setAttribute('aria-expanded', 'false');
      helpBtn?.focus();
    };
    const toggleHelp = () => {
      if (!helpEl) return;
      if (helpEl.hasAttribute('hidden')) openHelp();
      else closeHelp();
    };
    if (helpBtn) on(helpBtn, 'click', toggleHelp);
    if (helpClose) on(helpClose, 'click', closeHelp);
    if (helpEl) {
      on(helpEl, 'keydown', (ev) => {
        const ke = ev as KeyboardEvent;
        if (ke.key === 'Escape') {
          ke.stopPropagation();
          closeHelp();
          return;
        }
        if (ke.key !== 'Tab') return;
        const focusable = helpFocusables();
        const first = focusable[0];
        const lastEl = focusable[focusable.length - 1];
        if (!first || !lastEl) {
          ke.preventDefault();
          return;
        }
        if (ke.shiftKey) {
          if (document.activeElement === first) {
            ke.preventDefault();
            lastEl.focus();
          }
        } else if (document.activeElement === lastEl) {
          ke.preventDefault();
          first.focus();
        }
      });
    }

    on(window, 'keydown', (ev) => {
      const ke = ev as KeyboardEvent;
      const t = ke.target as HTMLElement | null;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) return;
      if (ke.metaKey || ke.ctrlKey || ke.altKey) return;
      if (helpEl && !helpEl.hasAttribute('hidden')) {
        if (ke.key === 'Escape') {
          ke.preventDefault();
          closeHelp();
        }
        return;
      }
      const [cx, cy] = stageCenter();
      const step = ke.shiftKey ? 160 : 64;
      switch (ke.key) {
        case 'ArrowLeft':
          tx += step;
          applyTransform();
          ke.preventDefault();
          break;
        case 'ArrowRight':
          tx -= step;
          applyTransform();
          ke.preventDefault();
          break;
        case 'ArrowUp':
          ty += step;
          applyTransform();
          ke.preventDefault();
          break;
        case 'ArrowDown':
          ty -= step;
          applyTransform();
          ke.preventDefault();
          break;
        case '+':
        case '=':
          zoomAt(cx, cy, 1.2);
          ke.preventDefault();
          break;
        case '-':
        case '_':
          zoomAt(cx, cy, 1 / 1.2);
          ke.preventDefault();
          break;
        case '0':
        case 'f':
        case 'F':
          fit();
          ke.preventDefault();
          break;
        case '?':
          toggleHelp();
          ke.preventDefault();
          break;
        case 'Escape': {
          const hadSelection = pinned !== null;
          pinned = null;
          hover = null;
          if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
          renderSidebar(null);
          applyView();
          if (hadSelection) announce('Selection cleared');
          ke.preventDefault();
          break;
        }
      }
    });

    on(window, 'resize', () => fit());

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
    fit();

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
            <defs>
              <marker
                id="ar-critical"
                markerWidth={9}
                markerHeight={9}
                refX={7.5}
                refY={3}
                orient="auto"
              >
                <path d="M0,0 L7,3 L0,6 Z" fill="#ff3860" />
              </marker>
              <marker
                id="ar-api"
                markerWidth={9}
                markerHeight={9}
                refX={7.5}
                refY={3}
                orient="auto"
              >
                <path d="M0,0 L7,3 L0,6 Z" fill="#ff7a45" />
              </marker>
              <marker id="ar-db" markerWidth={9} markerHeight={9} refX={7.5} refY={3} orient="auto">
                <path d="M0,0 L7,3 L0,6 Z" fill="#ffb86b" />
              </marker>
              <marker
                id="ar-mount"
                markerWidth={9}
                markerHeight={9}
                refX={7.5}
                refY={3}
                orient="auto"
              >
                <path d="M0,0 L7,3 L0,6 Z" fill="#4ea1ff" />
              </marker>
              <marker
                id="ar-normal"
                markerWidth={9}
                markerHeight={9}
                refX={7.5}
                refY={3}
                orient="auto"
              >
                <path d="M0,0 L7,3 L0,6 Z" fill="#3a3a3a" />
              </marker>
            </defs>
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
          <div className="legend" id="legend">
            <div className="row">
              <span className="sw sw-critical" /> critical path
            </div>
            <div className="row">
              <span className="sw sw-api" /> external / API call
            </div>
            <div className="row">
              <span className="sw sw-db" /> DB read/write
            </div>
            <div className="row">
              <span className="sw sw-mount" /> mount / register
            </div>
            <div className="row">
              <span className="dot dot-fix" /> fix count{'  '}
              <span className="dot dot-bug" /> bug count
            </div>
          </div>
          <div
            id="kbd-help"
            hidden
            role="dialog"
            aria-modal="true"
            aria-labelledby="kbd-help-title"
          >
            <div className="kbd-title">
              <span id="kbd-help-title">Keyboard shortcuts</span>
              <button id="kbd-help-close" type="button" aria-label="Close keyboard shortcuts">
                ✕
              </button>
            </div>
            <ul>
              <li>
                <kbd>drag</kbd> · <kbd>↑</kbd>
                <kbd>↓</kbd>
                <kbd>←</kbd>
                <kbd>→</kbd> pan
              </li>
              <li>
                <kbd>wheel</kbd> · <kbd>+</kbd>
                <kbd>−</kbd> zoom
              </li>
              <li>
                <kbd>Tab</kbd> focus next node
              </li>
              <li>
                <kbd>Enter</kbd> pin · <kbd>Esc</kbd> clear
              </li>
              <li>
                <kbd>0</kbd> / <kbd>F</kbd> fit · <kbd>?</kbd> toggle help
              </li>
            </ul>
          </div>
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
