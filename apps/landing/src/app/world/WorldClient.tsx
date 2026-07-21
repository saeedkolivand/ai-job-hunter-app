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

  // The vendored engine has NO teardown API: it registers window scroll/
  // resize/orientationchange/pointer listeners plus an unbounded rAF loop, and
  // never revokes the blob object URLs it creates per clip (intentional —
  // clips must stay seekable for the page's whole lifetime; don't "fix" that
  // upstream). mountedRef only guards StrictMode's double-invoke, not a route
  // remount, so /world must only be entered via a full navigation (raw <a>,
  // as the home body.html link is) — never an in-app Next <Link>, which would
  // stack a second listener set + rAF loop over the first render's detached DOM.
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
