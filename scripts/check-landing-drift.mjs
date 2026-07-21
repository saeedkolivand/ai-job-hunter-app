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
// Secret-scan (ALL landing html/js): no committed GitHub token — the site is public.
//
// Read-only. Run via `pnpm check:landing-drift`; CI runs it in the Lint & Format job.

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();

// Claim-bearing architecture diagrams (path/IPC/registry/denylist checks). The
// architecture map is a passthrough dashboard under public/; how-it-works is now
// a Next route whose authored body lives in src/content/.
//
// NOTE: the agent-system page was ported to a typed data source
// (src/data/agent-fleet.ts, PR2), but it is deliberately NOT in this path-checked
// set: its `paths` fields cite GLOB patterns (e.g. `apps/desktop/src-tauri/src/**`)
// that never resolve under the literal existsSync in checkPaths. It is secret-scanned
// below instead (see SECRET_SCAN_FILES). check-agent-system.mjs owns its name/roster
// invariants.
const DIAGRAMS = [
  'apps/landing/public/architecture-map.html',
  'apps/landing/src/content/how-it-works/body.html',
];

// Every authored landing page + embedded script (secret-scan only) — the site is
// public, so no committed token may ship. The ported pages' text now lives in
// src/content/*/body.html and their former inline scripts in public/scripts/*.js;
// the dashboards + benchmarks are public/ passthrough.
const SECRET_SCAN_FILES = [
  'apps/landing/src/data/agent-fleet.ts',
  'apps/landing/public/architecture-map.html',
  'apps/landing/public/benchmarks/index.html',
  'apps/landing/public/benchmarks/data.js',
  'apps/landing/social-card.html',
  'apps/landing/src/data/version.json',
  ...['home', 'creature', 'download', 'how-it-works', 'privacy'].map(
    (r) => `apps/landing/src/content/${r}/body.html`
  ),
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

// `N('ct-apply', 'contract', 'apply.ts', …)` → the 3rd arg is the contract file.
const CONTRACT_NODE_RE = /N\(\s*'[^']*'\s*,\s*'contract'\s*,\s*'([A-Za-z0-9]+)\.ts'/g;
// Any explicit `…/ipc/contracts/<name>.ts` reference.
const CONTRACT_PATH_RE = /ipc\/contracts\/([A-Za-z0-9]+)\.ts/g;

function checkContracts(file, text, valid) {
  const cited = new Set();
  for (const [, name] of text.matchAll(CONTRACT_NODE_RE)) cited.add(name);
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

for (const file of SECRET_SCAN_FILES) {
  if (existsSync(join(ROOT, file))) checkSecrets(file, read(file));
}

if (failures.length === 0) {
  console.log(
    '✓ apps/landing/ diagrams in sync with source (paths, IPC contracts, registries, no secrets)'
  );
  process.exit(0);
}

// Group the report by check, then file.
console.error(`✗ apps/landing/ drift detected — ${failures.length} issue(s):\n`);
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
