'use client';

// The #cookie notice from src/content/home/body.html. Its dismiss button carried
// the page's only inline handler (onclick="document.getElementById('cookie').remove()")
// — JSX can't express a string onclick, so it's an onClick handler here instead.
// The listener now attaches after hydration rather than being present in the
// served HTML; scripts/diff-dom.mjs skips on*-prefixed attributes for exactly
// this reason (ADR 0018 — everything else about the DOM must stay identical).
export function CookieGag() {
  return (
    <div id="cookie">
      we don't track you. we can barely track ourselves.
      <div>
        <button
          onClick={() => document.getElementById('cookie')?.remove()}
          aria-label="dismiss cookie notice"
        >
          ok
        </button>
      </div>
    </div>
  );
}
