import { Conformance } from './sections/Conformance';
import { Footer } from './sections/Footer';
import { InPlace } from './sections/InPlace';
import { Intro } from './sections/Intro';

// Body markup for /accessibility. New page (not a port of legacy HTML), split
// into sections/ the same way components/privacy/PrivacyBody.tsx is — purely
// for file size, no props — so it composes in document order. The "Partially
// conformant" wording in sections/Conformance.tsx is load-bearing; see
// AccessibilityBody.test.tsx's regression guard before changing it.
export function AccessibilityBody() {
  return (
    <div style={{ display: 'contents' }}>
      <main className="wrap">
        <Intro />
        <Conformance />
        <InPlace />
        <Footer />
      </main>
    </div>
  );
}
