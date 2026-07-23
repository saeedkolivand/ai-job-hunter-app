// Copy-parity gate for the Next port of the landing pages. Asserts, against the
// BUILT output in out/, that the faithful port did not silently drop any legacy
// link or signature joke. Three checks, all against out/ (never the source):
//
//   1. Signature phrases — a curated list of each page's distinctive copy/jokes
//      must each appear somewhere in the built output. Phrases that lived in the
//      old inline <script> gags now ship as /public/scripts/*.js (copied into
//      out/scripts/), so we scan out/**/*.{html,js}, not just HTML.
//   2. Legacy links — a curated, frozen list of every content href that lived in
//      the old src/content/*/body.html fragments (the /download version block
//      excluded — it's version-specific and validated by check 3). Frozen here
//      rather than derived because the src/content/ fragments are being deleted
//      page-by-page as this refactor converts them to TSX; the list was snapshot
//      once from the source fragments before conversion started.
//   3. Installer URLs — every per-OS URL in src/data/version.json must appear in
//      out/download.html (the download version seam actually rendered).
//
// Checks 1 and 2 normalize() the built HTML first: React's serializer entity-
// escapes text (' -> &#x27;, " -> &quot;, & -> &amp;), so a phrase/href with one
// of those characters no longer byte-matches raw JSX output without unescaping.
//
// Node stdlib only; exits nonzero on drift. Wired as `check:parity`; run after a
// build (`next build` emits out/).

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const appDir = join(here, '..');
const outDir = join(appDir, 'out');
const versionPath = join(appDir, 'src', 'data', 'version.json');

function normalize(s) {
  return s
    .replace(/&#x27;|&#39;/g, "'")
    .replace(/&quot;/g, '"')
    .replace(/&amp;/g, '&');
}

// ── Curated signature phrases — each page's distinctive copy/jokes ────────────
// Kept here (not derived) so a dropped joke fails loudly. Grouped by page.
const PHRASES = [
  // index (/) — hero, honesty block, footer, gag script
  'IT DOES EVERYTHING ELSE.',
  'please hire',
  'does everything but hit submit',
  'pulls from 24 boards',
  'PolyForm Noncommercial',
  'ok fine, take the app',
  "no, I still don't have a job",
  'buy me a coffee',
  'pure spite',
  'made by Saeed, between rejections.',
  // creature (/creature) — the short film
  'THE CREATURE',
  'a hand-drawn doodle short film',
  'tiny recruiter creature',
  'you found the margins',
  // download (/download)
  'Take the app',
  'code-signing certificates cost money',
  'The browser extension',
  'xattr -cr',
  'you opened the console, not the installer',
  'brew install --cask ai-job-hunter',
  // how-it-works (/how-it-works)
  'How It Works (End to End)',
  'you press send',
  // privacy (/privacy)
  'Privacy Policy',
  'we can barely track ourselves',
  'No accounts',
  '127.0.0.1',
  'still not tracking you',
];

// ── Frozen legacy link snapshot — taken from src/content/*/body.html before ───
// those fragments started getting deleted page-by-page for the TSX conversion.
// (download's version block was excluded when this was snapshotted — its URLs
// are version-specific and validated by check 3.)
const EXPECTED_HREFS = [
  '/creature',
  '/download',
  '/privacy',
  '/storybook/',
  '/world',
  'https://addons.mozilla.org/en-US/firefox/addon/ai-job-hunter-job-importer/',
  'https://apify.com/privacy-policy',
  'https://chromewebstore.google.com/detail/ai-job-hunter-%E2%80%94-job-impor/oaoekkgkhmgdfnpmfkpphgiikliaicll',
  'https://github.com/saeedkolivand/ai-job-hunter-app',
  'https://github.com/sponsors/saeedkolivand',
  'https://ko-fi.com/saeedkolivand',
  'https://paypal.me/saeedkolivand',
  'mailto:contact@aijobhunter.app',
];

// Every .html/.js under out/ (built pages + copied /public passthrough).
function readBuiltOutput() {
  const files = readdirSync(outDir, { recursive: true, withFileTypes: true });
  const parts = [];
  const byName = new Map();
  for (const d of files) {
    if (!d.isFile() || !/\.(html|js)$/.test(d.name)) continue;
    const full = join(d.parentPath ?? d.path, d.name);
    const text = readFileSync(full, 'utf8');
    parts.push(text);
    byName.set(full, text);
  }
  return { all: parts.join('\n'), byName };
}

// ── Run ───────────────────────────────────────────────────────────────────────
if (!existsSync(outDir)) {
  console.error('check:parity FAILED — out/ not found. Run `next build` first.');
  process.exit(1);
}

const built = readBuiltOutput();
const builtAll = normalize(built.all);
const errors = [];

for (const phrase of PHRASES) {
  if (!builtAll.includes(phrase)) errors.push(`signature phrase missing from out/: "${phrase}"`);
}

for (const href of EXPECTED_HREFS) {
  if (!builtAll.includes(href)) errors.push(`legacy link missing from out/: ${href}`);
}

// Installer URLs actually rendered on the built /download page.
const downloadHtml = normalize(
  [...built.byName].find(([name]) => /[/\\]download\.html$/.test(name))?.[1] ?? ''
);
const version = JSON.parse(readFileSync(versionPath, 'utf8'));
const installerUrls = Object.values(version.installers ?? {});
for (const url of installerUrls) {
  if (!downloadHtml.includes(url))
    errors.push(`installer URL missing from out/download.html: ${url}`);
}

// Required deploy-shape passthrough files.
const REQUIRED_FILES = [
  'CNAME',
  '.nojekyll',
  'og-card.jpg',
  'agent-system.html',
  'architecture-map.html',
  'benchmarks/index.html',
  'world.html',
];
for (const rel of REQUIRED_FILES) {
  if (!existsSync(join(outDir, rel))) errors.push(`passthrough file missing from out/: ${rel}`);
}

if (errors.length > 0) {
  console.error('check:parity FAILED — the built out/ dropped legacy content:');
  for (const e of errors) console.error(`  - ${e}`);
  console.error(
    `\n${errors.length} issue(s). Phrases checked: ${PHRASES.length}, links checked: ${EXPECTED_HREFS.length}, installer URLs: ${installerUrls.length}.`
  );
  process.exit(1);
}

console.log(
  `check:parity OK — ${PHRASES.length} signature phrases, ${EXPECTED_HREFS.length} legacy links, and ` +
    `${installerUrls.length} installer URLs all present in out/; deploy-shape files intact.`
);
