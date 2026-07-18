// Skip-link -- first in tab order. Target depends on which layer is actually
// live: the gl-live A11yOverlay region ("#tv-content") while GL runs, or the
// visible Semantic layer's main content ("#story-content") in fallback /
// pending / slideshow, where the overlay isn't mounted at all.

export function SkipLink({ href }: { href: string }) {
  return (
    <a className="skip-link" href={href}>
      Skip to content
    </a>
  );
}
