import { Doodles } from './sections/Doodles';
import { Overlays } from './sections/Overlays';
import { Stage } from './sections/Stage';

// Body markup for /creature, converted 1:1 from the deleted
// src/content/creature/body.html. creature-0.js (the SVG film engine) and
// creature-1.js bind to this DOM entirely by id (#stage, #caps, #fig, #hud,
// #controls, #progress, #titlecard, #endcard, #play, #pauseBtn, ...) — the
// rendered DOM must stay byte-identical (ADR 0018). The root
// <div style={{ display: 'contents' }}> replaces the old RawHtml wrapper so
// the serialized DOM is unchanged. The margin doodles, the stage/hud/
// controls/progress, and the titlecard/endcard/sitefoot overlays are split
// into src/components/creature/sections/ purely for file size — no props,
// same mechanical conversion; this file only composes them in the original
// DOM order.
export function CreatureBody() {
  return (
    <div style={{ display: 'contents' }}>
      <Doodles />
      <Stage />
      <Overlays />
    </div>
  );
}
