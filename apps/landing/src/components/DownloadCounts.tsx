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

// The second, independent figure this component fills in: one public
// "installs" total (GitHub installer downloads plus the Microsoft, Snap,
// Chrome and Firefox store counts — see scripts/lib/store-counts.mjs for the
// unit caveat, since a store "user" or "acquisition" isn't literally a
// download). It has its own fetch, its own try/catch and its own placeholder
// element, so a failure or absence here never touches the per-platform pills
// above and vice versa.
const STORE_COUNTS_URL = '/store-counts.json';

const SVG_NS = 'http://www.w3.org/2000/svg';

/**
 * Builds the inline "download" glyph shared by every badge on the page — the
 * GitHub per-platform pills and the four store pills alike, so there is one
 * badge mechanism, not two. `currentColor` so it always matches whatever text
 * colour the pill inherits (cream on .dl-btn/.ext-btn's dark fill, ink on
 * .dl-btn.alt/.store-btn's light one). Built via `createElementNS`, not
 * `innerHTML`, so nothing here is ever an HTML-injection sink even though the
 * markup is a fixed constant.
 */
function buildDownloadIcon(): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.setAttribute('class', 'dl-icon');
  svg.setAttribute('viewBox', '0 0 16 16');
  svg.setAttribute('aria-hidden', 'true');
  svg.setAttribute('focusable', 'false');

  const path = document.createElementNS(SVG_NS, 'path');
  path.setAttribute('d', 'M8 1.5v8M4.3 6.2 8 9.9l3.7-3.7M2.5 13.5h11');
  path.setAttribute('fill', 'none');
  path.setAttribute('stroke', 'currentColor');
  path.setAttribute('stroke-width', '1.6');
  path.setAttribute('stroke-linecap', 'round');
  path.setAttribute('stroke-linejoin', 'round');
  svg.appendChild(path);

  return svg;
}

/**
 * Builds one `.dl-count` pill — icon, the formatted number, and an sr-only
 * unit word — the one badge shape every count on the page uses. `title`
 * repeats the same pairing as a hover tooltip, so a store's unit is stated
 * honestly even for someone who never reaches the sr-only text.
 */
function buildCountPill(formatted: string, unitWord: string): HTMLSpanElement {
  const pill = document.createElement('span');
  pill.className = 'dl-count';
  pill.title = `${formatted} ${unitWord}`;
  pill.appendChild(buildDownloadIcon());

  const number = document.createElement('span');
  number.className = 'dl-count-number';
  number.textContent = formatted;
  pill.appendChild(number);

  const unit = document.createElement('span');
  unit.className = 'sr-only';
  unit.textContent = ` ${unitWord}`;
  pill.appendChild(unit);

  return pill;
}

// Each store's own unit — never "downloads", because a store figure isn't one
// (see scripts/lib/store-counts.mjs for the caveat). Keyed by the
// store-counts.json field name, which is also the `data-store` value
// DownloadCards (msStore, snap) and DownloadBody (chrome, firefox) put on
// that store's button.
const STORE_UNITS: Record<string, { one: string; many: string }> = {
  msStore: { one: 'acquisition', many: 'acquisitions' },
  snap: { one: 'install', many: 'installs' },
  chrome: { one: 'user', many: 'users' },
  firefox: { one: 'daily user', many: 'daily users' },
};

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

          // Without this the link announces as "Intel · .dmg 5", where the 5
          // reads as part of the file description. The unit has to be spoken.
          btn.appendChild(buildCountPill(format.format(n), n === 1 ? 'download' : 'downloads'));
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

  useEffect(() => {
    let cancelled = false;

    void (async () => {
      try {
        const res = await fetch(STORE_COUNTS_URL);
        if (!res.ok || cancelled) return;
        const data: unknown = await res.json();
        if (cancelled || typeof data !== 'object' || data === null) return;

        const store = data as Record<string, unknown>;
        const format = new Intl.NumberFormat('en-US');

        const total = store.total;
        if (typeof total === 'number' && Number.isFinite(total) && total >= 0) {
          const el = document.querySelector<HTMLElement>('[data-installs-total]');
          // StrictMode-safe: a second run must not re-fill an already-filled node.
          if (el && !el.textContent) {
            // Un-hide BEFORE filling the text: aria-live only announces
            // mutations of a region that is already rendered, so setting
            // textContent first would speak to nobody.
            el.hidden = false;
            el.textContent = `${format.format(total)} installs so far, counting GitHub downloads and the app and extension stores.`;
          }
        }

        // Per-store badges — same `.dl-count` pill as the per-platform ones
        // above, one `[data-store]` host per button (DownloadCards for MS
        // Store/Snap Store, DownloadBody for Chrome/Firefox). A null, missing,
        // or zero figure leaves the badge off rather than showing a stale or
        // zero number.
        document.querySelectorAll<HTMLElement>('[data-store]').forEach((host) => {
          if (host.querySelector('.dl-count')) return;

          const key = host.dataset.store;
          const unit = key === undefined ? undefined : STORE_UNITS[key];
          const n = key === undefined ? undefined : store[key];
          if (!unit || typeof n !== 'number' || !Number.isFinite(n) || n <= 0) return;

          host.appendChild(buildCountPill(format.format(n), n === 1 ? unit.one : unit.many));
        });
      } catch {
        // Silent, same contract as the per-platform fetch above: a missing or
        // malformed payload must never break the page, just leave badges off.
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  return null;
}
