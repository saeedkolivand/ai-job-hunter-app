import type { MapNode } from '@/data/architecture-map';

import {
  ensureVisible as ensureVisiblePure,
  fit as fitPure,
  type OnFn,
  screenToVB as screenToVBPure,
  stageCenter as stageCenterPure,
  type Viewport,
  WHEEL_ZOOM_FACTOR,
  zoomAt as zoomAtPure,
} from './geometry';

export interface ViewportControls {
  fit: () => void;
  zoomAt: (cx: number, cy: number, factor: number) => void;
  panBy: (dx: number, dy: number) => void;
  ensureVisible: (n: MapNode) => void;
  stageCenter: () => [number, number];
  screenToVB: (px: number, py: number) => { x: number; y: number };
  consumeDragMoved: () => boolean;
}

// Pan + zoom: imperative transform on the #vp group (no React state per wheel/
// pointer event → 60fps). Owns the only mutable tx/ty/scale; geometry.ts's pure
// functions compute the next value from it.
export function attachViewport(stage: HTMLDivElement, vp: SVGGElement, on: OnFn): ViewportControls {
  let state: Viewport = { tx: 0, ty: 0, scale: 1 };
  const applyTransform = () =>
    vp.setAttribute('transform', `translate(${state.tx} ${state.ty}) scale(${state.scale})`);

  const fit = () => {
    state = fitPure(stage.getBoundingClientRect());
    applyTransform();
  };
  const zoomAt = (cx: number, cy: number, factor: number) => {
    state = zoomAtPure(state, cx, cy, factor);
    applyTransform();
  };
  const panBy = (dx: number, dy: number) => {
    state = { ...state, tx: state.tx + dx, ty: state.ty + dy };
    applyTransform();
  };
  const ensureVisible = (n: MapNode) => {
    const next = ensureVisiblePure(state, stage.getBoundingClientRect(), n);
    if (next !== state) {
      state = next;
      applyTransform();
    }
  };
  const stageCenter = () => stageCenterPure(stage.getBoundingClientRect());
  const screenToVB = (px: number, py: number) =>
    screenToVBPure(stage.getBoundingClientRect(), px, py);

  on(
    stage,
    'wheel',
    (ev) => {
      const we = ev as WheelEvent;
      we.preventDefault();
      const factor = we.deltaY < 0 ? WHEEL_ZOOM_FACTOR : 1 / WHEEL_ZOOM_FACTOR;
      const p = screenToVB(we.clientX, we.clientY);
      zoomAt(p.x, p.y, factor);
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
    panBy(me.clientX - lx, me.clientY - ly);
    lx = me.clientX;
    ly = me.clientY;
  });
  on(window, 'mouseup', () => {
    dragging = false;
    stage.classList.remove('panning');
  });
  on(window, 'resize', () => fit());

  const consumeDragMoved = () => {
    if (!moved) return false;
    moved = false;
    return true;
  };

  return { fit, zoomAt, panBy, ensureVisible, stageCenter, screenToVB, consumeDragMoved };
}
