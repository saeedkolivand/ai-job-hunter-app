import { Beat1 } from './beats/Beat1';
import { Beat2 } from './beats/Beat2';
import { Beat3 } from './beats/Beat3';
import { Beat4 } from './beats/Beat4';

// beat1-4 sections of the home page — split out of HomeBody.tsx purely for
// file size (a mechanical 1:1 conversion of the four "BEAT" <section>s in the
// deleted src/content/home/body.html; no props). Beat1-4 themselves live in
// src/components/home/beats/, split further for file size. See HomeBody.tsx
// for the shared conversion notes and the ADR-0018 DOM-fidelity constraint
// that public/scripts/home-0.js depends on (poke-a-doodle via
// [data-scream]/[data-voice]/[data-lines], counters via [data-to], scroll-driven
// --p/--c scrubbing on .stage/.reveal).
export function HomeBeats() {
  return (
    <>
      <Beat1 />
      <Beat2 />
      <Beat3 />
      <Beat4 />
    </>
  );
}
