'use client';

import { useEffect, useRef } from 'react';

import { mountScrollWorld } from './scrub-engine';
import { withAv1Sources, WORLD_CONFIG } from './world-config';

// Theme tokens the vendored engine reads off .sw-root/:root (see scrub-engine.js's
// header comment). Page background matches --sw-bg so the still posters blend
// seamlessly with the surrounding page. Anton / Patrick Hand are self-hosted via
// public/fonts/fonts.css (injected by <Fonts /> in layout.tsx) — family names must
// match that stylesheet's @font-face declarations exactly.
const THEME_CSS = `
  .sw-root, :root {
    --sw-bg: #f4ecdc;
    --sw-ink: #1c1812;
    --sw-ink-soft: #6a6072;
    --sw-accent: #e24b4a;
    --sw-font-display: 'Anton', sans-serif;
    --sw-font-body: 'Patrick Hand', cursive;
  }
  html, body { background: #f4ecdc; }
`;

export function WorldClient() {
  const containerRef = useRef<HTMLDivElement>(null);

  // mountScrollWorld returns a disposer (an ADR-0019 deviation — upstream returns
  // nothing and never unregisters its window listeners or its rAF loop). Handing
  // that straight back from the effect is all the cleanup React needs: StrictMode's
  // dev double-invoke and a client-side route remount both unwind properly, so
  // /world no longer has to be entered via a full navigation.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    // Desktop ships AV1 (~40% smaller at equal quality) with the H.264 files as
    // the fallback for browsers without AV1 decode (older Safari); mobile stays
    // H.264-only because phone software AV1 decode can't keep up with scrubbing.
    const av1 =
      document.createElement('video').canPlayType('video/mp4; codecs="av01.0.08M.08"') !== '';
    return mountScrollWorld(container, av1 ? withAv1Sources(WORLD_CONFIG) : WORLD_CONFIG);
  }, []);

  return (
    <>
      <style>{THEME_CSS}</style>
      <div ref={containerRef} id="world" />
    </>
  );
}
