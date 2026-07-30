// Pure reveal-on-scroll / draw-on-scroll / count-up math. IntersectionObserver
// wiring + state lives in components/agent-system/hooks.ts (useReveal).

import { COUNT_UP_DURATION_MS } from './constants';

export function easeOutCubic(p: number): number {
  return 1 - Math.pow(1 - p, 3);
}

/**
 * Animates el's textContent from 0 to `to` over COUNT_UP_DURATION_MS with a
 * cubic-out ease. Synchronous (jumps straight to `to`) under reduced motion.
 */
export function countUp(el: HTMLElement, to: number, reduce: boolean): void {
  if (reduce) {
    el.textContent = String(to);
    return;
  }
  let start: number | null = null;
  function frame(t: number) {
    if (start === null) start = t;
    const p = Math.min(1, (t - start) / COUNT_UP_DURATION_MS);
    const eased = easeOutCubic(p);
    el.textContent = String(Math.round(eased * to));
    if (p < 1) window.requestAnimationFrame(frame);
    else el.textContent = String(to);
  }
  el.textContent = '0';
  window.requestAnimationFrame(frame);
}

/** Clamped 0-1 scroll progress of a `.draw` scene, used to drive its `--p` CSS var. */
export function sceneProgress(top: number, height: number, viewportHeight: number): number {
  const p = (viewportHeight - top) / (viewportHeight + height);
  return p < 0 ? 0 : p > 1 ? 1 : p;
}
