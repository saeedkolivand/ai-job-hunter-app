import { CookieGag } from './CookieGag';
import { HomeBeats } from './HomeBeats';
import { Features } from './sections/Features';
import { Finale } from './sections/Finale';
import { Growth } from './sections/Growth';
import { Hero } from './sections/Hero';
import { PageChrome } from './sections/PageChrome';
import { Testimonials } from './sections/Testimonials';

// Body shell for / (home), converted 1:1 from the deleted
// src/content/home/body.html. The rendered DOM must stay byte-identical —
// public/scripts/home-0.js (loader, scroll progress/odometer, the #journey SVG
// scrubbing, poke-a-doodle via [data-scream]/[data-voice]/[data-lines], the
// sound toggle, konami) binds to it by id/class/data-* (ADR 0018). The chrome
// and the original 8 <main> sections are split into
// src/components/home/sections/ and src/components/home/beats/ purely for
// file size — no props, same mechanical conversion; this file only composes
// them in the original DOM order. <Growth/> is a 9th section added later
// (self-updating star/download charts, not part of the 1:1 port) — it is
// still a direct `<main> > <section>` because #journey's line-drawing
// overlay (public/scripts/home-0.js:231) walks
// `document.querySelectorAll('main > section')` generically up to `.finale`
// with no hardcoded count or index, so appending a section here needs no
// script change. The root <div style={{display:'contents'}}> replaces the
// old RawHtml wrapper so the serialized DOM is unchanged for the ported
// sections. #cookie's only inline handler moved to CookieGag ('use client'),
// the one intentional deviation from the port.
export function HomeBody() {
  return (
    <div style={{ display: 'contents' }}>
      <PageChrome>
        <CookieGag />
      </PageChrome>

      <main>
        <Hero />
        <HomeBeats />
        <Features />
        <Testimonials />
        <Growth />
        <Finale />
      </main>
    </div>
  );
}
