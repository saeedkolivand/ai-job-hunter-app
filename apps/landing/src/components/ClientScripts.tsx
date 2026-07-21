'use client';

import { useEffect } from 'react';

// Runs each page's original inline gag scripts (console easter eggs, the doodle
// pokes, the creature/pipeline players) verbatim from /public/scripts. They are
// appended after mount so they execute against the already-rendered body markup,
// exactly as the inline <script> at the end of <body> used to. `async=false`
// preserves source order. Kept as external files (not typechecked TS) so the
// legacy JS — including one intentionally-preserved broken console egg on
// how-it-works — ships byte-identical.
//
// ORIGIN INVARIANT (load-bearing): every `src` here MUST be a first-party,
// same-origin `/scripts/*` file — NEVER a third-party URL. There is no
// third-party JavaScript anywhere on this origin, ever: any foreign script on
// any page can read the /mission-control PAT out of localStorage, and a per-page
// CSP cannot protect it. See CspMeta.tsx. `isAllowedScriptSrc` enforces this
// fail-closed at runtime, not by comment alone.
export function isAllowedScriptSrc(src: string): boolean {
  // Same-origin absolute path under /scripts/ only. Rejects third-party URLs,
  // protocol-relative `//host`, and anything not rooted at /scripts/.
  return src.startsWith('/scripts/');
}

export function ClientScripts({ srcs }: { srcs: readonly string[] }) {
  useEffect(() => {
    const appended: HTMLScriptElement[] = [];
    for (const src of srcs) {
      if (!isAllowedScriptSrc(src)) {
        // Dev: fail loudly so the mistake is caught in review/CI. Prod: skip the
        // foreign src rather than execute it (the origin invariant is absolute).
        if (process.env.NODE_ENV !== 'production') {
          throw new Error(`ClientScripts refused a non-/scripts/ src: ${src}`);
        }
        continue;
      }
      if (document.querySelector(`script[data-gag="${src}"]`)) continue;
      const el = document.createElement('script');
      el.src = src;
      el.async = false;
      el.dataset.gag = src;
      document.body.appendChild(el);
      appended.push(el);
    }
    // Cleanup only removes tags THIS invocation appended (the guard already
    // skips ones another invocation owns), so unmount doesn't leak nodes.
    // Under StrictMode dev double-invoke this fires synchronously right after
    // the first effect, before the browser has queued the dynamically-inserted
    // script's execution (an async task per the HTML spec, never synchronous
    // with appendChild) — removing the node here cancels that pending
    // execution, so the second effect's re-append is the only one that
    // actually runs. No double side effects, no leaked nodes.
    return () => {
      for (const el of appended) el.remove();
    };
  }, [srcs]);

  return null;
}
