import { Desktop } from './sections/Desktop';
import { Extension } from './sections/Extension';
import { Footer } from './sections/Footer';
import { IntroShort } from './sections/IntroShort';

// Body markup for /privacy, converted 1:1 from the deleted
// src/content/privacy/body.html. The rendered DOM must stay byte-identical —
// scripts/check-parity.mjs pins this page's copy and anchor hrefs (ADR 0018).
// Nothing scripts it: privacy-0.js is a console easter egg with no DOM
// binding. The root <div style={{display:'contents'}}> replaces
// the old RawHtml wrapper so the serialized DOM is unchanged. The <h2>-
// delimited blocks are split into src/components/privacy/sections/ purely
// for file size — no props, same mechanical conversion; this file only
// composes them in the original DOM order.
export function PrivacyBody() {
  return (
    <div style={{ display: 'contents' }}>
      <main className="wrap">
        <IntroShort />
        <Extension />
        <Desktop />
        <Footer />
      </main>
    </div>
  );
}
