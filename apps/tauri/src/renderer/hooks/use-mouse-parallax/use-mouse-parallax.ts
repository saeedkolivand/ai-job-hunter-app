import { useEffect, useRef } from 'react';

/**
 * Ref-based mouse parallax. Attach the returned ref to a container element; on
 * pointer movement the hook writes the normalized pointer offset (each axis in
 * `[-1..1]`, relative to viewport centre) to the CSS custom properties
 * `--parallax-x` / `--parallax-y` on that element — directly, via RAF-throttled
 * DOM mutation, with **no React state and no re-renders**.
 *
 * Children consume the vars in CSS, e.g.:
 *   `transform: translate3d(calc(var(--parallax-x) * 30px), calc(var(--parallax-y) * 20px), 0)`
 */
export function useMouseParallax<T extends HTMLElement = HTMLDivElement>() {
  const ref = useRef<T>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el || typeof window === 'undefined') return;

    let frame = 0;
    const onMove = (e: PointerEvent) => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        const x = (e.clientX / window.innerWidth) * 2 - 1;
        const y = (e.clientY / window.innerHeight) * 2 - 1;
        el.style.setProperty('--parallax-x', x.toFixed(4));
        el.style.setProperty('--parallax-y', y.toFixed(4));
        frame = 0;
      });
    };

    window.addEventListener('pointermove', onMove, { passive: true });
    return () => {
      window.removeEventListener('pointermove', onMove);
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);

  return ref;
}
