// PERMANENT, BLOCKING accessibility gate for the built landing site (out/).
//
// Why blocking (unlike every other a11y check in this repo): PR #924
// (2026-08-01) fixed every WCAG A/AA violation this project could find and
// published /accessibility, which STATES "0 violations across all 9 landing
// pages". Nothing re-checked that claim before this file existed — every
// #924 scan ran from a throwaway script under apps/desktop/, the only place
// @axe-core/playwright already resolved. Because the baseline is already 0,
// there is no backlog to burn down, so any new violation this gate finds IS
// a regression — it is allowed to fail the build. That is deliberately
// DIFFERENT from every other a11y check here, which stays advisory because
// its surface ISN'T clean yet:
//   - apps/desktop/e2e/a11y.spec.ts — logs violations, never fails the run
//   - eslint.a11y.config.mjs        — warn-only jsx-a11y pass, kept out of
//                                      the strict `--max-warnings 0` lint
//   - .lighthouserc.json            — accessibility score is a "warn" assertion
// Do not "harmonise" this file down to advisory to match those — they're
// advisory because their surface isn't clean; this one's baseline is 0, so
// blocking is the correct (and only honest) behaviour.
//
// How: serve the BUILT out/ with node:http (no server dep — mirrors
// check-parity.mjs), drive headless Chromium via Playwright + axe-core, and
// scan EVERY route discovered by globbing out/**/*.html — never a hardcoded
// list. A hardcoded list is the exact failure mode PR #924's review flagged
// (a new route had to be added by hand in four places to get covered at
// all); auto-discovery means a new page is covered the day it exists.
//
// Node stdlib + playwright + @axe-core/playwright only. Exits nonzero on ANY
// WCAG 2a/2aa/21a/21aa/22aa violation on ANY discovered route. Wired as
// `check:a11y`; run after a build (`next build` emits out/).

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { dirname, extname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import AxeBuilder from '@axe-core/playwright';
import { chromium } from 'playwright';

const here = dirname(fileURLToPath(import.meta.url));
const appDir = join(here, '..');
const outDir = join(appDir, 'out');

// The exact tag set /accessibility's numbers were measured with (see
// Conformance.tsx's "Known barriers" card). Do NOT widen this to axe's
// `best-practice` rules — the statement targets WCAG A/AA only, and the
// desktop home view has two known best-practice findings (landmark-unique,
// region) deliberately left out of scope. Widening here would fail this
// gate on things the published statement never claimed to have fixed.
const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

const VIEWPORT = { width: 1280, height: 900 };

// Reveal/scroll-driven pages need extra settle time after `load`: /world
// builds its entire DOM client-side (WorldClient mounts a vendored scroll-
// scrubbed engine in a useEffect — the static HTML ships an empty container)
// and /creature runs a doodle reveal sequence on mount. Scanning before that
// settles would flag the pre-mount/mid-transition state, not the real page.
// Keyed by route (what page.goto() uses), not file.
const SETTLE_MS = { '/world': 1500, '/creature': 1000 };
const DEFAULT_SETTLE_MS = 300;

// Pages intentionally excluded from the gate. Auto-discovery still finds
// them — nothing is hidden — they're just not held to WCAG A/AA here, each
// for a stated reason. A trailing '/' excludes the whole subtree. Add an
// entry ONLY with a comment explaining why: silent exclusion is the exact
// failure mode this file exists to catch. Disclose every exclusion on
// /accessibility too (Conformance.tsx's Known barriers section).
const EXCLUDED_ROUTES = [
  // Third-party output of `benchmark-action/github-action-benchmark`
  // v1.22.1, regenerated wholesale by the `benchmark` job in
  // .github/workflows/quality.yml (`benchmark-data-dir-path:
  // apps/landing/public/benchmarks`) on every push-to-main perf run — a fix
  // committed here would be silently overwritten by the next run, so it
  // isn't fixable in this repo. First scan (2026-08-01) found two
  // violations, both disclosed as a known barrier on /accessibility:
  // color-contrast on #dl-button (white on #3298dc, 3.15:1, needs 4.5:1)
  // and html-has-lang (missing lang on <html>).
  '/benchmarks/',
  // Storybook's own static build (`storybook build`, not `next build`) —
  // copied into out/storybook only at Pages-deploy time (pages.yml, which
  // runs AFTER this gate's `next build` step), so `next build` alone never
  // produces it and this script won't discover it in CI. Listed anyway for
  // anyone who builds the full deploy shape locally and runs this gate
  // against it: same category as /benchmarks/ — bundled third-party markup
  // we don't author, never audited.
  '/storybook/',
];

function isExcludedRoute(route) {
  return EXCLUDED_ROUTES.some((r) => route === r || (r.endsWith('/') && route.startsWith(r)));
}

const MIME_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.txt': 'text/plain; charset=utf-8',
  '.xml': 'application/xml; charset=utf-8',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.png': 'image/png',
  '.webp': 'image/webp',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
  '.woff': 'font/woff',
  '.mp4': 'video/mp4',
  '.mp3': 'audio/mpeg',
  '.glb': 'model/gltf-binary',
};

if (!existsSync(outDir)) {
  console.error('check:a11y FAILED — out/ not found. Run `next build` first.');
  process.exit(1);
}

// ── discover routes: glob out/**/*.html, map file path -> served URL ────────
// An `index.html` (at any depth) serves its directory ('/', '/benchmarks/');
// every other file serves its path minus '.html' ('/privacy', '/404',
// '/_not-found'). Mirrors how both this script's own server and GitHub Pages
// resolve a directory vs. a flat file (next.config.ts: trailingSlash: false).
function discoverRoutes() {
  const entries = readdirSync(outDir, { recursive: true, withFileTypes: true });
  const routes = [];
  for (const d of entries) {
    if (!d.isFile() || extname(d.name) !== '.html') continue;
    const full = join(d.parentPath ?? d.path, d.name);
    const rel = relative(outDir, full).replaceAll('\\', '/');
    const route =
      d.name === 'index.html'
        ? `/${rel.slice(0, -'index.html'.length)}`
        : `/${rel.slice(0, -'.html'.length)}`;
    routes.push({ route, file: full });
  }
  routes.sort((a, b) => a.route.localeCompare(b.route));
  return routes;
}

// ── static file server over out/ (root-relative asset paths only) ──────────
// The request path NEVER reaches a path expression. Walking out/ once up front
// and serving only from the resulting allowlist means traversal is impossible
// by construction, rather than by a check someone can later reorder or drop —
// `/../../etc/passwd` is simply not a key. (An earlier version built the path
// with join() and validated it afterwards; CodeQL flagged that as
// path-injection and was right to, since the tainted value still reached the
// sink. Removing the sink beats sanitising it.)
//
// Only real files are indexed, so a route whose .html sits beside a same-named
// directory of Next RSC prefetch payloads (out/privacy.html next to
// out/privacy/) can't resolve to the directory and blow up readFileSync.
function buildServableFiles() {
  const files = new Map();
  for (const d of readdirSync(outDir, { recursive: true, withFileTypes: true })) {
    if (!d.isFile()) continue;
    const abs = join(d.parentPath ?? d.path, d.name);
    const rel = relative(outDir, abs).replaceAll('\\', '/');
    files.set(`/${rel}`, abs); // /privacy.html, /fonts/fonts.css, /scripts/x.js
    if (!rel.endsWith('.html')) continue;
    files.set(`/${rel.slice(0, -'.html'.length)}`, abs); // /privacy (trailingSlash: false)
    if (d.name === 'index.html') files.set(`/${rel.slice(0, -'index.html'.length)}`, abs); // '/', '/benchmarks/'
  }
  return files;
}

const SERVABLE = buildServableFiles();

function resolveFile(urlPath) {
  return SERVABLE.get(decodeURIComponent(urlPath.split('?')[0].split('#')[0]));
}

const server = createServer((req, res) => {
  const file = resolveFile(req.url ?? '/');
  if (!file) {
    res.writeHead(404).end('not found');
    return;
  }
  res.writeHead(200, { 'Content-Type': MIME_TYPES[extname(file)] ?? 'application/octet-stream' });
  res.end(readFileSync(file));
});

await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;

// ── scan ──────────────────────────────────────────────────────────────────
const discovered = discoverRoutes();
const routes = discovered.filter(({ route }) => !isExcludedRoute(route));
const skipped = discovered.filter(({ route }) => isExcludedRoute(route));
if (skipped.length > 0) {
  console.log(
    `check:a11y — excluding ${skipped.length} route(s) from the gate (see EXCLUDED_ROUTES): ` +
      skipped.map((r) => r.route).join(', ')
  );
}

const browser = await chromium.launch();
// @axe-core/playwright refuses a bare browser.newPage() shorthand page (it
// needs the CDP session a real BrowserContext gives it) — one context, reused
// across routes since these are independent static pages with no shared state.
const context = await browser.newContext({ viewport: VIEWPORT });
const failures = []; // { route, violations }

try {
  for (const { route } of routes) {
    const page = await context.newPage();
    await page.goto(`${base}${route}`, { waitUntil: 'load' });
    await page.waitForTimeout(SETTLE_MS[route] ?? DEFAULT_SETTLE_MS);
    const { violations } = await new AxeBuilder({ page }).withTags(TAGS).analyze();
    await page.close();
    if (violations.length > 0) failures.push({ route, violations });
  }
} finally {
  await context.close();
  await browser.close();
  server.close();
}

// ── report ────────────────────────────────────────────────────────────────
// Actionable on its own, no re-run needed: page, rule, impact, node count,
// every failing selector, and — for color-contrast, the usual offender — the
// actual fg/bg and measured-vs-required ratio straight from axe's check data.
if (failures.length > 0) {
  console.error('check:a11y FAILED — WCAG A/AA violations found:\n');
  for (const { route, violations } of failures) {
    for (const v of violations) {
      console.error(
        `${route} — [${v.impact ?? 'n/a'}] ${v.id}: ${v.help} (${v.nodes.length} node(s))`
      );
      for (const node of v.nodes) {
        console.error(`    selector: ${node.target.join(' ')}`);
        for (const check of node.any) {
          if (check.id !== 'color-contrast' || !check.data) continue;
          const { fgColor, bgColor, contrastRatio, expectedContrastRatio } = check.data;
          console.error(
            `    fg ${fgColor} / bg ${bgColor} — measured ${contrastRatio}, needs ${expectedContrastRatio}`
          );
        }
      }
    }
    console.error('');
  }
  const violationCount = failures.reduce((n, f) => n + f.violations.length, 0);
  console.error(
    `${failures.length} page(s), ${violationCount} violation rule(s). check:a11y is BLOCKING — fix the ` +
      'regression, do not weaken or skip this gate.'
  );
  process.exit(1);
}

console.log(
  `check:a11y OK — ${routes.length} page(s) scanned, 0 WCAG A/AA violations` +
    (skipped.length > 0 ? ` (${skipped.length} page(s) excluded, see EXCLUDED_ROUTES).` : '.')
);
