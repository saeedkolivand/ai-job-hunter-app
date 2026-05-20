import { useEffect, useState } from 'react';

/**
 * Tracks pointer position as normalized [-1..1] values relative to viewport center.
 * Returns x, y values and a CSS-var binder for `--mx` / `--my` percentages.
 *
 * Use cases:
 *  - `transform: translate3d(calc(x * 30px), calc(y * 20px), 0)` for parallax orbs
 *  - `<div style={mouseVars}>` to drive `.bg-mouse-reactive`
 */
export function useMouseParallax(): {
  x: number;
  y: number;
  mouseVars: { '--mx': string; '--my': string };
} {
  const [pos, setPos] = useState({ x: 0, y: 0 });

  useEffect(() => {
    if (typeof window === 'undefined') return;

    let frame = 0;
    const onMove = (e: PointerEvent) => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        const x = (e.clientX / window.innerWidth) * 2 - 1;
        const y = (e.clientY / window.innerHeight) * 2 - 1;
        setPos({ x, y });
        frame = 0;
      });
    };
    window.addEventListener('pointermove', onMove, { passive: true });
    return () => {
      window.removeEventListener('pointermove', onMove);
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);

  const mouseVars = {
    '--mx': `${((pos.x + 1) / 2) * 100}%`,
    '--my': `${((pos.y + 1) / 2) * 100}%`,
  } as { '--mx': string; '--my': string };

  return { x: pos.x, y: pos.y, mouseVars };
}
