import type { MapNode } from '@/data/architecture-map';

// Fixed authoring canvas — the single source of truth for both the static
// <svg viewBox> (no-JS baseline) and fit()'s pixel-space scale math, so the two
// can never drift apart.
export const CANVAS = { w: 3060, h: 1330 };

// Zoom/pan tuning — named so the interaction code never carries an unexplained
// literal.
export const ZOOM_FACTOR = 1.2; // +/- keys and the zoom in/out buttons
export const WHEEL_ZOOM_FACTOR = 1.12; // mouse-wheel zoom increment
export const ZOOM_MIN = 0.15;
export const ZOOM_MAX = 4;
export const FIT_PADDING = 0.98; // fit() leaves a 2% margin around the canvas
export const ENSURE_VISIBLE_MARGIN = 90; // px kept clear of the stage edge on focus

// Attach/detach callback shape shared by every interaction module (viewport,
// keyboard) — the effect's on() cleanup-collector pattern is created once in
// interactions.ts and passed down as this type.
export type OnFn = <T extends EventTarget>(
  target: T,
  type: string,
  handler: EventListenerOrEventListenerObject,
  opts?: AddEventListenerOptions
) => void;

export interface Viewport {
  tx: number;
  ty: number;
  scale: number;
}

// ── deterministic geometry (ported verbatim from the original render engine) ──
// Edge paths are a pure function of the two nodes' fixed coordinates, so they are
// computed once here and rendered as static <path> markup (prerendered at build).
export function edgePath(a: MapNode, b: MapNode): { d: string; mx: number; my: number } {
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

// ── viewport math — pure functions of the current state + a plain rect, so the
// interaction layer owns the only mutable tx/ty/scale and derives the next value
// by calling these. Ported verbatim from the original effect; only
// getBoundingClientRect itself (done by the caller) was impure. ──────────────
export function fit(rect: { width: number; height: number }): Viewport {
  const scale = Math.min(rect.width / CANVAS.w, rect.height / CANVAS.h) * FIT_PADDING;
  return {
    scale,
    tx: (rect.width - CANVAS.w * scale) / 2,
    ty: (rect.height - CANVAS.h * scale) / 2,
  };
}

export function screenToVB(
  rect: { left: number; top: number },
  px: number,
  py: number
): { x: number; y: number } {
  return { x: px - rect.left, y: py - rect.top };
}

export function stageCenter(rect: { width: number; height: number }): [number, number] {
  return [rect.width / 2, rect.height / 2];
}

export function zoomAt(vp: Viewport, cx: number, cy: number, factor: number): Viewport {
  const next = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, vp.scale * factor));
  const k = next / vp.scale;
  return {
    scale: next,
    tx: cx - (cx - vp.tx) * k,
    ty: cy - (cy - vp.ty) * k,
  };
}

export function ensureVisible(
  vp: Viewport,
  rect: { width: number; height: number },
  n: MapNode
): Viewport {
  const m = ENSURE_VISIBLE_MARGIN;
  const cx = (n.x + n.w / 2) * vp.scale + vp.tx;
  const cy = (n.y + n.h / 2) * vp.scale + vp.ty;
  let dx = 0;
  let dy = 0;
  if (cx < m) dx = m - cx;
  else if (cx > rect.width - m) dx = rect.width - m - cx;
  if (cy < m) dy = m - cy;
  else if (cy > rect.height - m) dy = rect.height - m - cy;
  if (!dx && !dy) return vp;
  return { ...vp, tx: vp.tx + dx, ty: vp.ty + dy };
}
