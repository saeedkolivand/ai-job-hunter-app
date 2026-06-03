import { useEffect, useRef } from 'react';

import { useMouseParallax } from '@/hooks/use-mouse-parallax';
import { usePerformanceMode } from '@/store/preferences-store';

/**
 * Cinematic ambient backdrop.
 *
 * Layers (back → front):
 *   1. Aurora ribbons      — slow, wide, hue-rotating CSS keyframes
 *   2. Nebulae             — medium colorful blobs
 *   3. Soft streaks        — subtle horizontal light trails
 *   4. Cursor blob         — 900px lerp-smoothed glow that trails the mouse
 *   5. Floating glow orbs  — mouse parallax depth
 *   6. 64px grid texture   — radial-masked
 *   7. Film grain
 *   8. Radial vignette
 *
 * Performance:
 *   - Cursor blob uses a JavaScript lerp loop (not CSS transition) for organic
 *     smoothness; the transform is applied directly to the DOM node — zero React
 *     re-renders. The loop is paused while the tab/window is hidden.
 *   - Parallax orbs read `--parallax-x/y` written by {@link useMouseParallax} on
 *     the container ref — also zero re-renders (no state on pointer move).
 *   - `low-memory` renders nothing; `balanced` renders a reduced layer budget;
 *     `performance` adds the extra depth layers.
 */
export function CinematicBackground() {
  const mode = usePerformanceMode();
  const bgRef = useMouseParallax<HTMLDivElement>();

  // ── Lerp cursor blob ──────────────────────────────────────────────────────
  const blobRef = useRef<HTMLDivElement>(null);
  const targetRef = useRef({ x: 0, y: 0 }); // where the cursor IS
  const posRef = useRef({ x: 0, y: 0 }); // where the blob IS (lerped)
  const rafRef = useRef(0);
  const HALF = 450; // half of 900px blob
  const LERP = 0.005; // 0.5% per frame → extremely slow, dreamy floating effect

  useEffect(() => {
    // Skip RAF loop in low-memory mode — component returns null below.
    if (mode === 'low-memory') return;
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
  // Body is already #07060f in low-memory mode; skip all GPU layers.
  if (mode === 'low-memory') return null;

  // Extra depth layers only on the high-end budget; balanced trims them to cut
  // the number of large blurred/composited layers on the default mode.
  const full = mode === 'performance';

  return (
    <div ref={bgRef} className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
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
      {full && (
        <div
          className="absolute top-1/2 left-0 h-[45vh] w-[45vw] rounded-full opacity-25 animate-aurora-4"
          style={{
            background: 'radial-gradient(closest-side, var(--aurora-cyan) 0%, transparent 70%)',
            willChange: 'transform',
          }}
        />
      )}

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

      {/* Soft streaks */}
      <div className="light-streak absolute top-[20%] left-1/4 h-px w-[40vw] bg-white/10 animate-streak-1" />
      <div className="light-streak absolute top-[55%] left-1/3 h-px w-[35vw] bg-white/10 animate-streak-2" />
      {full && (
        <>
          <div className="light-streak absolute top-[75%] left-1/4 h-px w-[30vw] bg-white/5  animate-streak-3" />
          <div className="light-streak absolute top-[35%] left-1/2 h-px w-[25vw] bg-white/5  animate-streak-4" />
        </>
      )}

      {/* Cursor blob — 900px lerp-smoothed glow.
          Position is updated every RAF tick via ref mutation (no React renders).
          Gradient has three stops for a soft centre → hard falloff shape. */}
      <div
        ref={blobRef}
        className="cursor-glow absolute pointer-events-none"
        style={{
          width: 900,
          height: 900,
          top: 0,
          left: 0,
          background: [
            'radial-gradient(circle,',
            '  rgba(168,85,247,0.30)  0%,',
            '  rgba(168,85,247,0.14) 30%,',
            '  rgba(99,102,241,0.06) 55%,',
            '  transparent           72%)',
          ].join(' '),
          filter: 'blur(55px)',
          willChange: 'transform',
        }}
      />

      {/* Floating glow orbs — parallax driven by the container's --parallax-x/y
          CSS vars (written on pointer move via the ref, no React re-renders). */}
      <div
        className="glow-orb absolute top-[15%] right-[10%] h-40 w-40 rounded-full transition-transform duration-75"
        style={{
          transform:
            'translate3d(calc(var(--parallax-x, 0) * 30px), calc(var(--parallax-y, 0) * 20px), 0)',
          background:
            'radial-gradient(circle, rgba(168,85,247,0.35) 0%, rgba(168,85,247,0.12) 50%, transparent 75%)',
          filter: 'blur(12px)',
        }}
      />
      <div
        className="glow-orb absolute bottom-[12%] left-[8%] h-32 w-32 rounded-full transition-transform duration-75"
        style={{
          transform:
            'translate3d(calc(var(--parallax-x, 0) * -25px), calc(var(--parallax-y, 0) * 15px), 0)',
          background:
            'radial-gradient(circle, rgba(99,102,241,0.35) 0%, rgba(99,102,241,0.12) 50%, transparent 75%)',
          filter: 'blur(10px)',
        }}
      />
      {full && (
        <div
          className="glow-orb absolute top-[40%] left-[55%] h-24 w-24 rounded-full transition-transform duration-75"
          style={{
            transform:
              'translate3d(calc(var(--parallax-x, 0) * 15px), calc(var(--parallax-y, 0) * -10px), 0)',
            background:
              'radial-gradient(circle, rgba(168,85,247,0.2) 0%, rgba(99,102,241,0.08) 50%, transparent 75%)',
            filter: 'blur(8px)',
          }}
        />
      )}

      {/* Grid + grain + vignette */}
      <div className="absolute inset-0 bg-grid-texture" />
      <div className="absolute inset-0 bg-film-grain" />
      <div className="absolute inset-0 bg-radial-vignette" />
    </div>
  );
}
