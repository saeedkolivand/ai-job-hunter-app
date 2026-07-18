"use client";

// The accessible interface while GL runs. Visually hidden but focusable and
// screen-reader operable: real anchors for the projector-slate menu + skip-to-end,
// and an aria-live region mirroring the current act (the letterbox caption text).
// The canvas itself stays aria-hidden; interactive controls are never canvas-only.

import { MENU_LINKS } from "@/content/story";
import { SCENES } from "@/engine/scene-resolver";
import { useRig } from "@/engine/store";

export function A11yOverlay({ onSkipToEnd }: { onSkipToEnd: () => void }) {
  // scene is a discrete store field (updated only on scene change), so this
  // re-renders a handful of times per session -- never per frame.
  const scene = useRig((s) => s.scene);
  const act = SCENES[scene]?.act ?? "";

  return (
    <section id="tv-content" className="a11y-overlay" aria-label="Film controls">
      <h2>AI Job Hunter -- the film</h2>
      <p aria-live="polite" aria-atomic="true">
        Now playing: {act}
      </p>
      <nav aria-label="Film menu">
        <ul>
          {MENU_LINKS.map((l) =>
            l.external ? (
              <li key={l.href}>
                <a href={l.href} target="_blank" rel="noopener noreferrer">
                  {l.label}
                </a>
              </li>
            ) : (
              <li key={l.href}>
                <a href={l.href}>{l.label}</a>
              </li>
            ),
          )}
          <li>
            {/* #credits lives inside the Semantic layer, which stays
                visibility:hidden + inert while gl-live -- an anchor jump
                would land keyboard/SR focus on an invisible, unfocusable
                node. Drive the playhead to the credits scene instead. */}
            <button type="button" onClick={onSkipToEnd}>
              skip to end (credits)
            </button>
          </li>
        </ul>
      </nav>
    </section>
  );
}
