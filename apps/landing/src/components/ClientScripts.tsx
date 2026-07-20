'use client';

import { useEffect } from 'react';

// Runs each page's original inline gag scripts (console easter eggs, the doodle
// pokes, the creature/pipeline players) verbatim from /public/scripts. They are
// appended after mount so they execute against the already-rendered body markup,
// exactly as the inline <script> at the end of <body> used to. `async=false`
// preserves source order. Kept as external files (not typechecked TS) so the
// legacy JS — including one intentionally-preserved broken console egg on
// how-it-works — ships byte-identical.
export function ClientScripts({ srcs }: { srcs: readonly string[] }) {
  useEffect(() => {
    const appended: HTMLScriptElement[] = [];
    for (const src of srcs) {
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
