'use client';

import { useEffect } from 'react';

// Cumulative per-platform installer downloads, rendered as a pill on each
// download button.
//
// SAME-ORIGIN AND STATIC, not the GitHub API. The honest figure is cumulative
// across every release, and `releases/latest` only ever reports the newest one
// — where every installer reads 1, because each asset picks up one automated
// download nobody performed. Computing the real number client-side would mean
// paginating the entire release list on every page view, against a 60/hour
// budget shared by all visitors. Instead 📈 Repo Charts computes it nightly
// (scripts/lib/github-releases.mjs, downloadsByPlatform) and pages.yml copies
// the result into public/ next to the growth charts.
//
// Injected into the DOM rather than rendered as JSX because DownloadCards is a
// server component whose markup is held to the ADR-0018 DOM-fidelity contract;
// DownloadFreshness already mutates the same buttons in place on this page, so
// this follows the idiom that is already here rather than adding a second one.
const COUNTS_URL = '/downloads-by-platform.json';

export function DownloadCounts() {
  useEffect(() => {
    let cancelled = false;

    void (async () => {
      try {
        const res = await fetch(COUNTS_URL);
        if (!res.ok || cancelled) return;
        const counts: unknown = await res.json();
        if (cancelled || typeof counts !== 'object' || counts === null) return;

        const byPlatform = counts as Record<string, unknown>;
        const format = new Intl.NumberFormat('en-US');

        document.querySelectorAll<HTMLAnchorElement>('.dl-btn[data-platform]').forEach((btn) => {
          // Effects run twice under StrictMode in dev; without this the pill
          // would be appended once per run.
          if (btn.querySelector('.dl-count')) return;

          const key = btn.dataset.platform;
          const n = key === undefined ? undefined : byPlatform[key];
          if (typeof n !== 'number' || !Number.isFinite(n) || n < 0) return;

          const pill = document.createElement('span');
          pill.className = 'dl-count';
          pill.textContent = format.format(n);

          // Without this the link announces as "Intel · .dmg 5", where the 5
          // reads as part of the file description. The unit has to be spoken.
          const unit = document.createElement('span');
          unit.className = 'sr-only';
          unit.textContent = n === 1 ? ' download' : ' downloads';
          pill.appendChild(unit);

          btn.appendChild(pill);
        });
      } catch {
        // Silent, like DownloadFreshness: a missing count must never cost
        // someone the download button it sits on.
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  return null;
}
