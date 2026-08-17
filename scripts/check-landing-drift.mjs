// Drift guard for the hand-authored landing diagrams, so they can never silently
// lie about the architecture (cf. gen-workflow-catalog.mjs — "never drift from reality").
//
// The two interactive diagrams embed factual claims (file paths, IPC contract names,
// registry references) plus curated prose. Nothing regenerates them, so when source
// moves — e.g. the apply→assist pivot removed `applying/` — they rot. This validator
// reads them as text and fails CI when a claim no longer matches the live source.
//
// Checks (architecture diagrams):
//   1. Every repo-relative path the markup cites exists on disk.
//   2. Every cited IPC contract namespace exists under packages/shared/src/ipc/contracts/.
//   3. No reference to the removed auto-apply registry (APPLIERS / &dyn Applier).
//   4. Forbidden-term denylist for the removed engine (anchored; the verb "applies" is fine).
//   5. Every board-count claim matches the live SCRAPERS registry entry count.
//   6. No stale "zero/no telemetry" privacy claim (ADR-0020 made it untrue).
//   7. Every publicly-named third party in the egress inventory
//      (apps/desktop/src-tauri/tests/egress.rs) is named, as a whole word, on
//      the actual disclosure surfaces (README.md, SECURITY.md, the /privacy
//      page) — a presence floor, not a semantic check.
//   8. No over-absolute "the only network calls…" / "sends … only to…" /
//      "no data leaves … except…" family of claim — the shape ADR-0005
//      exists because of.
//   9. The vendored chart.js matches its pinned sha256 (catches a silent edit
//      or re-vendor to a different, unverified build).
// Secret-scan (ALL landing html/js): no committed GitHub token — the site is public.
//
// Check 6 reaches beyond apps/landing/ — to README.md, SECURITY.md and branding/ —
// because a privacy claim drifts across every surface that repeats it, not just the
// site. Hosted here rather than in a new script so there is one place to look for
// "did the copy stop matching the code".
//
// Read-only. Run via `pnpm check:landing-drift`; CI runs it in the Lint & Format job.

import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();

// Claim-bearing architecture diagrams (path/IPC/registry/denylist checks). The
// architecture map is now a Next route backed by a typed data module
// (src/data/architecture-map.ts) whose nodes cite REAL file paths + IPC contract
// names — exactly what this guard validates; how-it-works is a Next route whose
// authored body is a 1:1-ported TSX component (src/components/how-it-works/
// HowItWorksBody.tsx, formerly the plain-text src/content/how-it-works/body.html
// — the checks below scan raw text, so the class="→className=" / quote-style
// rewrite from the TSX port doesn't change what they match).
//
// NOTE: the agent-system page was ported to a typed data source
// (src/data/agent-fleet.ts, PR2), but it is deliberately NOT in this path-checked
// set: its `paths` fields cite GLOB patterns (e.g. `apps/desktop/src-tauri/src/**`)
// that never resolve under the literal existsSync in checkPaths. It is secret-scanned
// below instead (see SECRET_SCAN_FILES). check-agent-system.mjs owns its name/roster
// invariants.
const DIAGRAMS = [
  'apps/landing/src/data/architecture-map.ts',
  'apps/landing/src/components/how-it-works/HowItWorksBody.tsx',
];

// Every authored landing page + embedded script (secret-scan only) — the site is
// public, so no committed token may ship. The five ported pages' authored text
// now lives in TSX body components under src/components/<slug>/ (formerly
// src/content/*/body.html, deleted across #872/#879/this branch's ports —
// that stale slug-template mapping is exactly how this scan silently voided
// itself for all five pages: existsSync-guarded, so a moved source failed
// open instead of loud). Kept as an explicit list, not a glob: checkPaths
// (above) only walks DIAGRAMS, so nothing else validates these literals —
// see the run loop below, which now hard-fails a missing entry instead of
// skipping it.
const SECRET_SCAN_FILES = [
  'apps/landing/src/data/agent-fleet.ts',
  'apps/landing/src/data/architecture-map.ts',
  'apps/landing/public/benchmarks/index.html',
  'apps/landing/public/benchmarks/data.js',
  'apps/landing/public/benchmarks/chart.min.js',
  'scripts/assets/social-card.html',
  'apps/landing/src/data/version.json',
  'apps/landing/src/components/home/HomeBody.tsx',
  'apps/landing/src/components/home/HomeBeats.tsx',
  'apps/landing/src/components/home/CookieGag.tsx',
  'apps/landing/src/components/home/sections/PageChrome.tsx',
  'apps/landing/src/components/home/sections/Hero.tsx',
  'apps/landing/src/components/home/sections/Features.tsx',
  'apps/landing/src/components/home/sections/Testimonials.tsx',
  'apps/landing/src/components/home/sections/Finale.tsx',
  'apps/landing/src/components/home/beats/Beat1.tsx',
  'apps/landing/src/components/home/beats/Beat2.tsx',
  'apps/landing/src/components/home/beats/Beat3.tsx',
  'apps/landing/src/components/home/beats/Beat4.tsx',
  'apps/landing/src/components/creature/CreatureBody.tsx',
  'apps/landing/src/components/creature/sections/Doodles.tsx',
  'apps/landing/src/components/creature/sections/Stage.tsx',
  'apps/landing/src/components/creature/sections/Overlays.tsx',
  'apps/landing/src/components/download/DownloadBody.tsx',
  'apps/landing/src/components/download/DownloadCards.tsx',
  'apps/landing/src/components/how-it-works/HowItWorksBody.tsx',
  'apps/landing/src/components/how-it-works/sections/Sidebar.tsx',
  'apps/landing/src/components/how-it-works/sections/Overview.tsx',
  'apps/landing/src/components/how-it-works/sections/Boot.tsx',
  'apps/landing/src/components/how-it-works/sections/Flows.tsx',
  'apps/landing/src/components/how-it-works/sections/IpcReference.tsx',
  'apps/landing/src/components/how-it-works/sections/Subsystems.tsx',
  'apps/landing/src/components/how-it-works/sections/CheatSheet.tsx',
  'apps/landing/src/components/privacy/PrivacyBody.tsx',
  'apps/landing/src/components/privacy/sections/IntroShort.tsx',
  'apps/landing/src/components/privacy/sections/Extension.tsx',
  'apps/landing/src/components/privacy/sections/Desktop.tsx',
  'apps/landing/src/components/privacy/sections/Footer.tsx',
  'apps/landing/src/components/accessibility/AccessibilityBody.tsx',
  'apps/landing/src/components/accessibility/sections/Intro.tsx',
  'apps/landing/src/components/accessibility/sections/Conformance.tsx',
  'apps/landing/src/components/accessibility/sections/InPlace.tsx',
  'apps/landing/src/components/accessibility/sections/Footer.tsx',
  'apps/landing/src/components/SiteFooter.tsx',
  'apps/landing/src/components/BackLink.tsx',
  'apps/landing/src/lib/site-links.ts',
  ...[
    'home-0',
    'creature-0',
    'creature-1',
    'download-0',
    'how-it-works-0',
    'how-it-works-1',
    'privacy-0',
  ].map((s) => `apps/landing/public/scripts/${s}.js`),
];

const IPC_CONTRACTS_DIR = 'packages/shared/src/ipc/contracts';
const SCRAPERS_FILE = 'apps/desktop/src-tauri/src/scraping/boards/mod.rs';

/** Collected failures, grouped by check for a readable report. */
const failures = [];
const fail = (check, file, detail) => failures.push({ check, file, detail });

const read = (rel) => readFileSync(join(ROOT, rel), 'utf8');

// ── Check 1: cited file paths exist ─────────────────────────────────────────
// Single- or double-quoted strings rooted at a real top-level dir. Strip a
// trailing `:<line>` locator; existsSync resolves both files and directories.
const PATH_RE = /['"]((?:apps|packages|scripts|docs)\/[^'"\s]+)['"]/g;

function checkPaths(file, text) {
  const seen = new Set();
  for (const [, raw] of text.matchAll(PATH_RE)) {
    const path = raw.replace(/:\d+$/, '').replace(/\/$/, '');
    if (seen.has(path)) continue;
    seen.add(path);
    if (!existsSync(join(ROOT, path))) {
      fail('Missing file paths', file, `cites '${raw}' — no such file or directory`);
    }
  }
}

// ── Check 2: cited IPC contract namespaces exist ────────────────────────────
function validContractNames() {
  // Every .ts in the dir is a valid citation target, including the `index.ts`
  // barrel (the architecture map references it). Test files are not contracts.
  return new Set(
    readdirSync(join(ROOT, IPC_CONTRACTS_DIR))
      .filter((f) => f.endsWith('.ts') && !f.endsWith('.test.ts'))
      .map((f) => f.replace(/\.ts$/, ''))
  );
}

// A contract-cluster node names its contract file via `label: '<name>.ts'`. Rather
// than assume property order or a restricted name charset (the old single regex
// required `cluster:` immediately before `label:` and [A-Za-z0-9] names, so a
// reordered property or an _/- name evaded it silently), scan each flat object
// literal and, for blocks tagged `cluster: 'contract'`, read the label. The
// serialized nodes nest no braces (arrays use []), so /\{[^{}]*?\}/ reliably
// bounds one node; `[\w-]+` covers underscore/hyphen names.
const CONTRACT_BLOCK_RE = /\{[^{}]*?\}/gs;
const CONTRACT_CLUSTER_RE = /cluster:\s*['"]contract['"]/;
const CONTRACT_LABEL_RE = /label:\s*['"]([\w-]+)\.ts['"]/;
// Any explicit `…/ipc/contracts/<name>.ts` reference.
const CONTRACT_PATH_RE = /ipc\/contracts\/([A-Za-z0-9]+)\.ts/g;

function checkContracts(file, text, valid) {
  const cited = new Set();
  for (const [block] of text.matchAll(CONTRACT_BLOCK_RE)) {
    if (!CONTRACT_CLUSTER_RE.test(block)) continue;
    const label = block.match(CONTRACT_LABEL_RE);
    if (label) cited.add(label[1]); // non-.ts labels (schemas/, types/) don't match
  }
  for (const [, name] of text.matchAll(CONTRACT_PATH_RE)) cited.add(name);
  for (const name of cited) {
    if (!valid.has(name)) {
      fail(
        'Unknown IPC contract',
        file,
        `cites contract '${name}.ts' — not in ${IPC_CONTRACTS_DIR}/`
      );
    }
  }
}

// ── Check 3: removed auto-apply registry ────────────────────────────────────
// The APPLIERS registry was deleted in the apply→assist pivot; SCRAPERS is the
// only board registry now. Read it so the anchor fails loudly if it ever moves.
const REGISTRY_RE = /\bAPPLIERS\b|&dyn\s+Applier\b|\bApplierRegistry\b|applying::/g;

function checkRegistry(file, text) {
  if (!existsSync(join(ROOT, SCRAPERS_FILE))) {
    fail('Registry source moved', file, `expected SCRAPERS registry at ${SCRAPERS_FILE}`);
    return;
  }
  for (const [match] of text.matchAll(REGISTRY_RE)) {
    fail(
      'Removed apply registry',
      file,
      `references '${match}' — the auto-apply registry was removed (use SCRAPERS / autopilot)`
    );
  }
}

// ── Check 4: forbidden-term denylist ────────────────────────────────────────
// Anchored so the legitimate verb "applies"/"apply" in the assist model is fine.
const DEAD_TERMS = [
  /applying\//g,
  /\bauto-apply\b/g,
  /\bapply_start\b/g,
  /\bapply_catalog\b/g,
  /\bapply\.step\b/g,
  /\bapply\.progress\b/g,
  /\bApplyContract\b/g,
];

function checkDeadTerms(file, text) {
  const hits = new Set();
  for (const re of DEAD_TERMS) {
    for (const [match] of text.matchAll(re)) hits.add(match);
  }
  for (const term of hits) {
    fail(
      'Removed apply engine term',
      file,
      `mentions '${term}' — a removed auto-apply concept; re-author for the assist model`
    );
  }
}

// ── Check 5: board-count claims match the SCRAPERS registry ─────────────────
// The landing sources state the board count in human-readable spots (the scraper
// cluster label, the FINDINGS prose, and the sidebar "N boards" line). The source
// of truth is the `static SCRAPERS` array in mod.rs — count its entries and assert
// every claim matches. NOTE: the sibling "8 AI providers" claim is intentionally
// NOT derived here — it comes from the ProviderId registry, a different source and
// out of scope for this guard.
const BOARD_COUNT_FILES = [
  'apps/landing/src/data/architecture-map.ts',
  'apps/landing/src/components/architecture-map/ArchitectureMap.tsx',
];
const BOARD_CLAIM_RES = [
  /SCRAPERS · (\d+)/g, // cluster label: "Scrapers (SCRAPERS · 24)"
  /SCRAPERS \((\d+)\)/g, // FINDINGS prose: "SCRAPERS (24)"
  /(\d+) boards/g, // node subs + sidebar: "24 boards"
];

// Entries of `static SCRAPERS: &[…] = &[ … ];` — one `&FooScraper,` per line.
function countScrapers(text) {
  const block = text.match(/static\s+SCRAPERS\b[^=]*=\s*&\[([\s\S]*?)\];/);
  if (!block) return null;
  return (block[1].match(/^\s*&\w+Scraper,\s*$/gm) ?? []).length;
}

function checkBoardCount(file, text, expected) {
  for (const re of BOARD_CLAIM_RES) {
    for (const [, n] of text.matchAll(re)) {
      if (Number(n) !== expected) {
        fail(
          'Board-count drift',
          file,
          `claims ${n} boards but the SCRAPERS registry has ${expected} (${SCRAPERS_FILE})`
        );
      }
    }
  }
}

// ── Check 6: no "zero/no telemetry" claim outside the extension ─────────────
// ADR-0020 added Sentry crash reporting (desktop, default ON, opt-out), which
// reversed a published "no telemetry" promise. The PROSE in README.md and
// SECURITY.md was rewritten at the time, but three SUMMARY claims were missed —
// README's Privacy bullet, the landing home page's Privacy-first card, and the
// marketing asset copy — so the repo contradicted itself, in one case ~70 lines
// apart in the same file, with nothing to catch it. This check is that catch.
//
// The claim is still TRUE of the browser extension: it is excluded from Sentry
// and declares `data_collection_permissions: { required: ['none'] }` to Firefox
// AMO. `apps/extension/**` is therefore deliberately NOT scanned, and the one
// extension-scoped file inside a scanned root is allowlisted below.
//
// Everywhere else, say "no analytics" or name the crash report — the canonical
// vocabulary is the "Crash reporting" entry in docs/CONTEXT.md, which reserves
// "telemetry"/"analytics" for behavioural and usage events (none are collected)
// and keeps them distinct from failure reports (which are).
const TELEMETRY_CLAIM_RE = /\b(?:zero|no)[\s-]+telemetry\b/gi;

// Files/dirs carrying user-facing privacy summaries. Roots rather than an
// explicit file list so a NEW marketing surface is covered on the day it lands —
// an explicit list only guards the three spots we already know about.
const TELEMETRY_SCAN_ROOTS = [
  'README.md',
  'SECURITY.md',
  'branding',
  'apps/landing/src',
  'apps/landing/public/llms.txt',
];

const TELEMETRY_TEXT_EXT = /\.(md|mdx|tsx?|mjs|js|txt|html)$/i;

// The regex is deliberately blunt, so it also hits prose that NARRATES the
// reversal rather than claiming it. Those files are named here individually —
// never a directory, or the guard starts failing open on whole trees.
const TELEMETRY_ALLOWED = new Set([
  // Extension-scoped section of the privacy page — true, and load-bearing for
  // the AMO declaration above.
  'apps/landing/src/components/privacy/sections/Extension.tsx',
  // The tech-radar entry for Sentry, whose rationale reads "reversed a published
  // no-telemetry promise" — a description of ADR-0020, not a promise. Its own
  // accuracy is covered by the radar staleness check.
  'apps/landing/src/data/tech-radar.ts',
]);

/** Text files under `rel`, recursively; `rel` may itself be a file. */
function textFilesUnder(rel) {
  const abs = join(ROOT, rel);
  if (!existsSync(abs)) return null;
  const entries = (() => {
    try {
      return readdirSync(abs, { withFileTypes: true });
    } catch {
      return null; // not a directory → a single file
    }
  })();
  if (entries === null) return TELEMETRY_TEXT_EXT.test(rel) ? [rel] : [];
  return entries.flatMap((e) => {
    if (e.name === 'node_modules' || e.name === 'out' || e.name === 'dist') return [];
    return textFilesUnder(`${rel}/${e.name}`) ?? [];
  });
}

function checkTelemetryClaim() {
  for (const root of TELEMETRY_SCAN_ROOTS) {
    const files = textFilesUnder(root);
    if (files === null) {
      // Fail loud rather than open: a moved root would silently void the scan.
      fail(
        'Privacy-claim scan source moved',
        root,
        'listed in TELEMETRY_SCAN_ROOTS but no longer exists — update the list'
      );
      continue;
    }
    for (const file of files) {
      if (TELEMETRY_ALLOWED.has(file)) continue;
      for (const [match] of read(file).matchAll(TELEMETRY_CLAIM_RE)) {
        fail(
          'Stale no-telemetry claim',
          file,
          `claims '${match}' — untrue since ADR-0020 added opt-out crash reporting. ` +
            `Say "no analytics", or name the crash report (see docs/CONTEXT.md)`
        );
      }
    }
  }
}

// ── Check 7: egress disclosure — every publicly-named third party from the
// egress inventory must be named, as a whole word, on the actual disclosure
// surfaces ───────────────────────────────────────────────────────────────
// apps/desktop/src-tauri/tests/egress.rs's `EGRESS` const marks each outbound
// host `public_name: Some("…")` when that third party must be disclosed by
// name. This is a PRESENCE FLOOR — a page could still name "Exa" in an
// unrelated sentence and pass — but it must be an actual word match, not a
// bare substring: a case-sensitive `includes()` over all of apps/landing/src
// let 'Exa' pass on "Exact-pinned"/"Example]" and let 'GitHub'/'IMAP' pass on
// any mention anywhere in the tree (mission-control's own GitHub-API code,
// unrelated to privacy disclosure), so deleting the real disclosure stayed
// green. Fixed two ways: \b-anchored per-name regex, AND a corpus narrowed to
// where a disclosure would actually live — README, SECURITY, and the
// /privacy page's source (PrivacyBody.tsx + sections/) — not every .ts/.tsx
// file under apps/landing/src. Check 8 below (the banned-phrase guard) is the
// other half of the pair: this check catches an omission, that one catches a
// false claim of completeness.
const EGRESS_FILE = 'apps/desktop/src-tauri/tests/egress.rs';
const EGRESS_DISCLOSURE_SURFACES = [
  'README.md',
  'SECURITY.md',
  'apps/landing/src/components/privacy',
];
// Two shapes, deliberately: `EGRESS` rows carry `public_name: Option<&str>`
// (`Some("…")`), while `UNEXTRACTABLE` rows carry a bare `public_name: "…"`
// because there is no host string to make optional — Sentry's ingest host
// lives inside a build-time-secret DSN and exists in no reproducible build.
// Matching only the `Some(…)` form would leave the ONE default-ON automatic
// egress unenforced here, which is the exact gap this check exists to close.
const PUBLIC_NAME_RE = /public_name:\s*(?:Some\(\s*)?"([^"]+)"/g;
const escapeRegExp = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

function checkEgressDisclosure() {
  const names = new Set();
  for (const [, name] of read(EGRESS_FILE).matchAll(PUBLIC_NAME_RE)) names.add(name);
  if (names.size === 0) {
    // Fail loud rather than open — mirrors check 5's countScrapers self-guard:
    // a regex that silently stops matching would turn this into a permanently
    // green no-op, worse than not having the check at all.
    fail(
      'Egress-disclosure parse failed',
      EGRESS_FILE,
      'found zero `public_name` rows (EGRESS or UNEXTRACTABLE) — the extractor regex no longer matches; update it'
    );
    return;
  }
  // Test/spec sources are excluded from the corpus. `textFilesUnder` recurses
  // the whole privacy/ directory, which also holds `PrivacyBody.test.tsx` — and
  // a vendor named only in a Vitest file is not disclosed to anybody. Verified:
  // planting a probe name in that spec alone satisfied the presence floor and
  // the check stayed green, which is the same one-level-too-high narrowing this
  // check was already fixed for once.
  const isTestSource = (p) => /\.(test|spec)\.[cm]?[jt]sx?$/.test(p);
  const files = EGRESS_DISCLOSURE_SURFACES.flatMap((root) => {
    const found = textFilesUnder(root)?.filter((p) => !isTestSource(p)) ?? null;
    if (found === null) {
      // Fail loud rather than open — same idiom as checks 5/6/8: a moved
      // disclosure surface silently shrinking the corpus is the exact bug
      // this check was just fixed for.
      fail(
        'Egress-disclosure scan source moved',
        root,
        'listed in EGRESS_DISCLOSURE_SURFACES but no longer exists — update the list'
      );
      return [];
    }
    return found;
  });
  const prose = files.map(read).join('\n');
  for (const name of names) {
    if (!new RegExp(String.raw`\b${escapeRegExp(name)}\b`).test(prose)) {
      fail(
        'Undisclosed third party',
        EGRESS_FILE,
        `'${name}' carries public_name in the egress inventory but does not appear as a whole word ` +
          `in README.md, SECURITY.md, or apps/landing/src/components/privacy — name it, e.g. in ` +
          `apps/landing/src/components/privacy/sections/Desktop.tsx`
      );
    }
  }
}

// ── Check 8: over-absolute egress-summary phrase guard ──────────────────────
// The 2026-07 audit (ADR-0005) found exactly this sentence shape false once
// already: "the only network calls are X" / "no network calls other than X"
// goes stale the next time a new integration ships, because it claims
// completeness instead of describing what's disclosed. Same banned-phrase
// idiom as check 6's TELEMETRY_CLAIM_RE. A mutation review found the original
// single regex caught only that one literal shape — 2 of 10 realistic
// rewordings, missing both live instances in README.md/SECURITY.md ("sends
// data only to services you configure or invoke", "the one thing the app
// sends on its own behalf is…"). Broadened to the shape family below; each
// pattern is deliberately anchored (a generic-data noun for the "sends…only
// to…" shape, an "other"/exception-clause requirement for the "no…calls"
// shape) so a true, narrowly-scoped claim elsewhere on the site — "sends that
// HTML only to the local app" (Extension.tsx), "no network call is ever
// made" (one specific write-action) — does not also trip it.
const EGRESS_PROSE_ROOTS = ['README.md', 'SECURITY.md', 'apps/landing/src'];
const ABSOLUTE_EGRESS_CLAIM_RES = [
  // "(the) only network/outbound calls/connections/requests/traffic"
  /\b(?:the )?only\s+(?:network|outbound)\s+(?:calls?|connections?|requests?|traffic)\b/gi,
  // "no other network/outbound X" OR "no network/outbound X other than/except/besides"
  /\bno\s+other\s+(?:network|outbound)\s+(?:calls?|connections?|requests?|traffic)\b|\bno\s+(?:network|outbound)\s+(?:calls?|connections?|requests?|traffic)\b[^.]{0,40}?\b(?:other than|except|besides)\b/gi,
  // "(the) only {calls|requests} the app makes"
  /\bonly\s+(?:calls?|requests?)\s+the\s+app\s+makes\b/gi,
  // "sends … data/information/traffic … only to …"
  /\bsends?\b[^.]{0,20}?\b(?:data|information|traffic)\b[^.]{0,40}?\bonly\s+to\b/gi,
  // "the one thing … sends … is" — completeness phrased as a singleton
  /\bthe\s+on(?:e|ly)\s+thing\b[^.]{0,60}?\bsends?\b/gi,
  // "no <noun> leaves your device/machine/computer except/other than/besides"
  /\bno\s+\w+\s+leaves?\s+your\s+(?:device|machine|computer)\b[^.]{0,40}?\b(?:except|other than|besides)\b/gi,
];

function checkAbsoluteEgressClaim() {
  for (const root of EGRESS_PROSE_ROOTS) {
    const files = textFilesUnder(root);
    if (files === null) {
      fail(
        'Egress-claim scan source moved',
        root,
        'listed in EGRESS_PROSE_ROOTS but no longer exists'
      );
      continue;
    }
    for (const file of files) {
      const text = read(file);
      for (const re of ABSOLUTE_EGRESS_CLAIM_RES) {
        for (const [match] of text.matchAll(re)) {
          fail(
            'Over-absolute egress claim',
            file,
            `contains '${match}' — this sentence shape goes false the moment a new integration ` +
              `ships (ADR-0005); describe what's disclosed without claiming completeness`
          );
        }
      }
    }
  }
}

// ── Check 9: vendored chart.js hash pin ─────────────────────────────────────
// chart.min.js is hand-vendored (see its own header comment) rather than
// CDN-loaded, so nothing but a human re-fetching it ever changes these bytes.
// Pin the sha256 here so a silent edit/replacement — accidental or not — is
// caught in CI instead of shipping unnoticed. Bump this hash only alongside a
// deliberate re-vendor (update the header comment's own verified hash too).
const CHART_JS_FILE = 'apps/landing/public/benchmarks/chart.min.js';
const CHART_JS_SHA256 = '9f8701efa23e00ec6779325eb85d77bc101ebf65e37df5faa1966270e7da5c37';

function checkChartJsPin() {
  // Fail loud, never open. An earlier revision returned silently here on the
  // claim that SECRET_SCAN_FILES carried the moved-source guard — it did not
  // list this file, so deleting the very asset this pin exists to protect made
  // the check pass. It is in that list now (so the run loop's hard-fail covers
  // a move) and this branch is the belt: an integrity pin that goes quiet when
  // its subject vanishes is worse than no pin, because the green tick still
  // reads as "verified".
  if (!existsSync(join(ROOT, CHART_JS_FILE))) {
    fail(
      'Vendored chart.js missing',
      CHART_JS_FILE,
      'the pinned vendored asset is gone — restore it, or remove the pin and its ' +
        'SECRET_SCAN_FILES entry together if the page genuinely no longer needs chart.js'
    );
    return;
  }
  const actual = createHash('sha256').update(read(CHART_JS_FILE)).digest('hex');
  if (actual !== CHART_JS_SHA256) {
    fail(
      'Vendored chart.js hash drift',
      CHART_JS_FILE,
      `sha256 is ${actual}, expected ${CHART_JS_SHA256} — if this is a deliberate re-vendor, ` +
        `update CHART_JS_SHA256 (and the file's own header comment) together`
    );
  }
}

// ── Secret-scan: no committed GitHub token on the public site ───────────────
const TOKEN_RE = /\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}\b|\bgithub_pat_[A-Za-z0-9_]{20,}\b/g;

function checkSecrets(file, text) {
  for (const [match] of text.matchAll(TOKEN_RE)) {
    const masked = `${match.slice(0, 7)}…(redacted)`;
    fail(
      'Committed GitHub token',
      file,
      `contains what looks like a GitHub token '${masked}' — never commit tokens to a public page`
    );
  }
}

// ── Run ─────────────────────────────────────────────────────────────────────
const validContracts = validContractNames();

for (const file of DIAGRAMS) {
  const text = read(file);
  checkPaths(file, text);
  checkContracts(file, text, validContracts);
  checkRegistry(file, text);
  checkDeadTerms(file, text);
}

if (!existsSync(join(ROOT, SCRAPERS_FILE))) {
  fail('Registry source moved', SCRAPERS_FILE, `expected SCRAPERS registry for board-count check`);
} else {
  const boardCount = countScrapers(read(SCRAPERS_FILE));
  if (boardCount === null) {
    fail(
      'Registry source moved',
      SCRAPERS_FILE,
      'could not find the `static SCRAPERS` array to count'
    );
  } else {
    for (const file of BOARD_COUNT_FILES) checkBoardCount(file, read(file), boardCount);
  }
}

checkTelemetryClaim();
checkEgressDisclosure();
checkAbsoluteEgressClaim();
checkChartJsPin();

for (const file of SECRET_SCAN_FILES) {
  if (!existsSync(join(ROOT, file))) {
    fail(
      'Secret-scan source moved',
      file,
      'listed in SECRET_SCAN_FILES but no longer exists — update the list ' +
        '(this used to fail open silently, voiding the scan for that file)'
    );
    continue;
  }
  checkSecrets(file, read(file));
}

if (failures.length === 0) {
  console.log(
    '✓ landing diagrams in sync with source (paths, IPC contracts, registries, no secrets), ' +
      'no stale no-telemetry claim, and egress disclosure matches the EGRESS inventory'
  );
  process.exit(0);
}

// Group the report by check, then file.
console.error(`✗ copy drift detected — ${failures.length} issue(s):\n`);
const byCheck = new Map();
for (const f of failures) {
  if (!byCheck.has(f.check)) byCheck.set(f.check, []);
  byCheck.get(f.check).push(f);
}
for (const [check, items] of byCheck) {
  console.error(`  ${check}:`);
  for (const { file, detail } of items) console.error(`    - ${file}: ${detail}`);
  console.error('');
}
console.error(
  'Fix: update the landing diagram(s) to match current source, or correct the cited reference.\n' +
    'These pages are owned by project-steward — see docs-standards (Code → docs map).'
);
process.exit(1);
