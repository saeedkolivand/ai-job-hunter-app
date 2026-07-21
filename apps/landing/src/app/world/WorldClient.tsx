'use client';

import { useEffect, useRef } from 'react';

import { mountScrollWorld } from './scrub-engine';
import { WORLD_CONFIG } from './world-config';

// Theme tokens the vendored engine reads off .sw-root/:root (see scrub-engine.js's
// header comment). Page background matches --sw-bg so the still posters blend
// seamlessly with the surrounding page. Anton / Patrick Hand are self-hosted via
// public/fonts/fonts.css (injected by <Fonts /> in page.tsx) — family names must
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
  // React 19 StrictMode double-invokes effects in dev. The engine wires up
  // window-level scroll/resize listeners it never tears down, so re-running it
  // would double-register them (not just duplicate DOM) — a ref flag, not a
  // cleanup return, is what actually prevents that.
  const mountedRef = useRef(false);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || mountedRef.current) return;
    mountedRef.current = true;
    mountScrollWorld(container, WORLD_CONFIG);
  }, []);

  return (
    <>
      <style>{THEME_CSS}</style>
      <div ref={containerRef} id="world" />
    </>
  );
}
