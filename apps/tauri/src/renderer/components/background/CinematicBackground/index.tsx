import { useEffect, useRef } from 'react';

import { usePerformanceMode } from '@/store/preferences-store';

/**
 * Slim accent aurora backdrop.
 *
 * Layers (back → front):
 *   1. Aurora ribbons — three slow, wide, hue-rotating CSS blobs.
 *   2. Nebulae        — one (balanced) / two (performance) medium blobs.
 *   3. Cursor glow    — a 900px lerp-smoothed blob that trails the pointer.
 *
 * All colors derive from the accent: the aurora/nebula vars in tokens.css map
 * to the brand family, and the cursor glow mixes --color-brand / --color-brand-2,
 * so the whole layer re-tints with the chosen accent. Reduced motion disables
 * the aurora/nebula keyframes in CSS, and the cursor-glow RAF loop is also
 * skipped in JS (the glow is painted once, static, at viewport center).
 *
 * Performance:
 *   - The cursor glow uses a JavaScript lerp loop (not a CSS transition) applied
 *     straight to the DOM node — zero React re-renders — and is paused while the
 *     tab/window is hidden.
 *   - `low-memory` renders nothing; `balanced` trims to a reduced layer budget;
 *     `performance` adds the second nebula.
 */
export function CinematicBackground() {
  const mode = usePerformanceMode();

  // ── Lerp cursor glow ────────────────────────────────────────────────────────
  const blobRef = useRef<HTMLDivElement>(null);
  const targetRef = useRef({ x: 0, y: 0 }); // where the cursor IS
  const posRef = useRef({ x: 0, y: 0 }); // where the blob IS (lerped)
  const rafRef = useRef(0);
  const HALF = 450; // half of 900px blob
  const LERP = 0.005; // 0.5% per frame → extremely slow, dreamy floating effect

  useEffect(() => {
    // Skip RAF loop in low-memory mode — component returns null below.
    if (mode === 'low-memory') return;
    // Reduced-motion users get a static glow: paint the blob once at viewport
    // center and skip the pointer listener + RAF entirely (the aurora/nebula
    // keyframes are already neutralized by the CSS reduced-motion media query).
    const reduceMotion =
      typeof window !== 'undefined' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduceMotion) {
      const cx = window.innerWidth / 2;
      const cy = window.innerHeight / 2;
      if (blobRef.current) {
        blobRef.current.style.transform = `translate(${cx - HALF}px, ${cy - HALF}px)`;
      }
      return;
    }
    // Seed starting position to viewport center so blob doesn't slide in from (0,0)
    if (typeof window !== 'undefined') {
      const cx = window.innerWidth / 2;
      const cy = window.innerHeight / 2;
      targetRef.current = { x: cx, y: cy };
      posRef.current = { x: cx, y: cy };
    }

    const onMove = (e: PointerEvent) => {
      targetRef.current = { x: e.clientX, y: e.clientY };
    };

    const tick = () => {
      posRef.current.x += (targetRef.current.x - posRef.current.x) * LERP;
      posRef.current.y += (targetRef.current.y - posRef.current.y) * LERP;

      if (blobRef.current) {
        blobRef.current.style.transform = `translate(${posRef.current.x - HALF}px, ${posRef.current.y - HALF}px)`;
      }

      rafRef.current = requestAnimationFrame(tick);
    };

    // Pause the loop while hidden (background tab / minimized window) so we don't
    // burn frames animating something nobody can see; resume on return.
    const onVisibility = () => {
      cancelAnimationFrame(rafRef.current);
      if (!document.hidden) rafRef.current = requestAnimationFrame(tick);
    };

    window.addEventListener('pointermove', onMove, { passive: true });
    document.addEventListener('visibilitychange', onVisibility);
    if (!document.hidden) rafRef.current = requestAnimationFrame(tick);

    return () => {
      window.removeEventListener('pointermove', onMove);
      document.removeEventListener('visibilitychange', onVisibility);
      cancelAnimationFrame(rafRef.current);
    };
  }, [mode]); // re-run if mode changes so RAF is cleaned up correctly

  // All hooks have been called — safe to bail out now.
  if (mode === 'low-memory') return null;

  // The second nebula only renders on the high-end budget; balanced trims it to
  // cut the number of large blurred/composited layers on the default mode.
  const full = mode === 'performance';

  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
      {/* Aurora ribbons */}
      <div
        className="absolute -top-1/4 left-0 h-[60vh] w-[60vw] rounded-full opacity-40 animate-aurora-1"
        style={{
          background: 'radial-gradient(closest-side, var(--aurora-violet) 0%, transparent 70%)',
          willChange: 'transform',
        }}
      />
      <div
        className="absolute -top-1/4 left-0 h-[55vh] w-[55vw] rounded-full opacity-35 animate-aurora-2"
        style={{
          background: 'radial-gradient(closest-side, var(--aurora-indigo) 0%, transparent 70%)',
          willChange: 'transform',
        }}
      />
      <div
        className="absolute top-1/3 left-0 h-[40vh] w-[40vw] rounded-full opacity-25 animate-aurora-3"
        style={{
          background: 'radial-gradient(closest-side, var(--aurora-pink) 0%, transparent 70%)',
          willChange: 'transform',
        }}
      />

      {/* Nebulae */}
      <div
        className="absolute top-[10vh] left-0 h-[30vh] w-[30vw] rounded-full opacity-40 animate-nebula-1"
        style={{
          background: 'radial-gradient(closest-side, var(--nebula-violet) 0%, transparent 70%)',
          willChange: 'transform',
        }}
      />
      {full && (
        <div
          className="absolute top-[60vh] left-0 h-[25vh] w-[25vw] rounded-full opacity-35 animate-nebula-2"
          style={{
            background: 'radial-gradient(closest-side, var(--nebula-indigo) 0%, transparent 70%)',
            willChange: 'transform',
          }}
        />
      )}

      {/* Cursor glow — 900px lerp-smoothed blob.
          Position is updated every RAF tick via ref mutation (no React renders).
          Mixes the accent brand + gradient-end so it re-tints with the accent. */}
      <div
        ref={blobRef}
        className="cursor-glow absolute pointer-events-none"
        style={{
          width: 900,
          height: 900,
          top: 0,
          left: 0,
          background:
            'radial-gradient(circle, color-mix(in srgb, var(--color-brand) 30%, transparent) 0%, color-mix(in srgb, var(--color-brand) 14%, transparent) 30%, color-mix(in srgb, var(--color-brand-2) 6%, transparent) 55%, transparent 72%)',
          filter: 'blur(55px)',
          willChange: 'transform',
        }}
      />
    </div>
  );
}
