// Staleness guard for apps/landing/src/data/tech-radar.ts (the /tech-radar
// page — a curated, human-judged Adopt/Trial/Assess/Hold list, NOT derived
// from package.json: the whole point of a radar is the judgment call, and
// this repo already learned the hard way (four stale hardcoded AI-model
// defects in one session, see docs/adr/0022) that a curated list left
// unchecked rots silently until someone ships a bug from it.
//
// The asymmetry that makes this check correct rather than a repeat of that
// mistake: ADR-0022 explicitly REJECTED a CI staleness job for AI-model
// arrays because `RATES` deliberately retains RETIRED rows (so historical
// spend still prices correctly) and CLI-agent aliases appear in no catalogue
// — a checker there would cry wolf on almost every entry. A tech-radar entry
// has no such excuse: if it's tagged `subjectKind: 'dependency'`, the entry
// itself is asserting "this package exists in our stack today" — that's
// either true (checkable) or it's stale (fix it or delete the entry). The
// escape hatch for anything that ISN'T that claim — a technique, a hosted
// service, or something deliberately never adopted — is `subjectKind` itself
// (see tech-radar.ts's header comment): only 'dependency' entries are ever
// checked, so a Hold entry about a rejected package (XState, Codecov) can
// never trip this check by design, not by omission.
//
// Two checks:
//   1. Every `subjectKind: 'dependency'` entry's dependencyName (or `name` if
//      omitted) must be a real dependency key in a package.json / Cargo.toml
//      on disk today.
//   2. Every `adrSlug` must name a real file under docs/adr/.
//
// Read-only. Wired as `pnpm check:tech-radar`; runs in ci-pipeline.yml's
// Lint & Format job and in the pre-push hook.

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const RADAR_FILE = 'apps/landing/src/data/tech-radar.ts';
const ADR_DIR = 'docs/adr';
const CARGO_FILE = 'apps/desktop/src-tauri/Cargo.toml';

const read = (rel) => readFileSync(join(ROOT, rel), 'utf8');
const exists = (rel) => existsSync(join(ROOT, rel));

// ── Extract radar entries from the .ts source AS TEXT ───────────────────────
// Not imported/type-checked (that's `pnpm typecheck`'s job) — read as data,
// same convention as scripts/check-landing-drift.mjs reading architecture-map.ts.
// Unlike that file's flat "no braces in strings" convention, entry prose here
// is allowed to contain literal `{}` (e.g. "an inline { duration, ease }
// object"), so blocks are bounded with a string-aware brace-depth walk rather
// than a naive non-nested-brace regex.

/** Every `{ id: '...', ... }`-shaped top-level block in `source`, verbatim. */
function extractObjectBlocks(source) {
  const startRe = /\{\s*\n\s*id:\s*'/g;
  const blocks = [];
  let m;
  while ((m = startRe.exec(source))) {
    const start = m.index;
    let depth = 0;
    let inString = null; // the quote char currently open, or null
    let i = start;
    for (; i < source.length; i++) {
      const ch = source[i];
      if (inString) {
        if (ch === '\\') {
          i++; // skip the escaped character
          continue;
        }
        if (ch === inString) inString = null;
        continue;
      }
      if (ch === "'" || ch === '"') {
        inString = ch;
        continue;
      }
      if (ch === '{') depth++;
      else if (ch === '}') {
        depth--;
        if (depth === 0) {
          i++;
          break;
        }
      }
    }
    blocks.push(source.slice(start, i));
    startRe.lastIndex = i;
  }
  return blocks;
}

/** A `key: 'value'` / `key: "value"` field, anchored to its own line. */
function field(block, key) {
  const re = new RegExp(`^\\s*${key}:\\s*(?:'((?:[^'\\\\]|\\\\.)*)'|"((?:[^"\\\\]|\\\\.)*)")`, 'm');
  const m = block.match(re);
  if (!m) return undefined;
  return (m[1] ?? m[2] ?? '').replace(/\\(.)/g, '$1');
}

function parseRadarEntries(source) {
  return extractObjectBlocks(source)
    .filter((block) => /^\s*subjectKind:\s*['"]/m.test(block)) // excludes QUADRANTS/RINGS objects
    .map((block) => ({
      id: field(block, 'id'),
      name: field(block, 'name'),
      subjectKind: field(block, 'subjectKind'),
      dependencyName: field(block, 'dependencyName'),
      adrSlug: field(block, 'adrSlug'),
    }));
}

// ── Collect every dependency name declared anywhere in the repo ─────────────
const PKG_JSON_FIELDS = [
  'dependencies',
  'devDependencies',
  'peerDependencies',
  'optionalDependencies',
];

function packageJsonFiles() {
  const files = ['package.json'];
  for (const group of ['apps', 'packages']) {
    if (!exists(group)) continue;
    for (const dir of readdirSync(join(ROOT, group), { withFileTypes: true })) {
      if (!dir.isDirectory()) continue;
      const rel = `${group}/${dir.name}/package.json`;
      if (exists(rel)) files.push(rel);
    }
  }
  return files;
}

function collectPackageJsonDeps(names) {
  for (const file of packageJsonFiles()) {
    const json = JSON.parse(read(file));
    for (const fieldName of PKG_JSON_FIELDS) {
      for (const name of Object.keys(json[fieldName] ?? {})) names.add(name);
    }
  }
}

// Minimal Cargo.toml dependency-table scanner: tracks the current `[section]`
// header and records a bare `name = ...` key while inside any section whose
// path ends in "dependencies" (covers [dependencies], [dev-dependencies],
// [build-dependencies], and every per-target
// [target.'cfg(...)'.dependencies] table) — comment lines are skipped first
// so a `#`-prefixed line can never be misread as a dependency declaration.
function extractCargoDeps(text) {
  const names = new Set();
  let inDepsSection = false;
  for (const rawLine of text.split('\n')) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;
    const section = line.match(/^\[+([^\]]+)\]+$/);
    if (section) {
      inDepsSection = /dependencies$/.test(section[1]);
      continue;
    }
    if (!inDepsSection) continue;
    const dep = line.match(/^"?([A-Za-z0-9_.-]+)"?\s*=/);
    if (dep) names.add(dep[1]);
  }
  return names;
}

function collectCargoDeps(names) {
  if (!exists(CARGO_FILE)) return;
  for (const name of extractCargoDeps(read(CARGO_FILE))) names.add(name);
}

function collectKnownDependencyNames() {
  const names = new Set();
  collectPackageJsonDeps(names);
  collectCargoDeps(names);
  return names;
}

// ── Run ───────────────────────────────────────────────────────────────────
if (!exists(RADAR_FILE)) {
  console.error(`check:tech-radar FAILED — ${RADAR_FILE} not found.`);
  process.exit(1);
}

const entries = parseRadarEntries(read(RADAR_FILE));
if (entries.length === 0) {
  console.error(
    `check:tech-radar FAILED — parsed zero entries out of ${RADAR_FILE}. Either the file is empty or the parser's block-matching regex (extractObjectBlocks in this script) no longer matches the data file's formatting — fix whichever one drifted.`
  );
  process.exit(1);
}

const knownDeps = collectKnownDependencyNames();
const errors = [];

for (const entry of entries) {
  const label = entry.name ?? entry.id ?? '(unnamed entry)';

  if (entry.subjectKind === 'dependency') {
    const depName = entry.dependencyName ?? entry.name;
    if (!depName) {
      errors.push(
        `"${label}" has subjectKind: 'dependency' but no dependencyName (and no name) to check.`
      );
    } else if (!knownDeps.has(depName)) {
      errors.push(
        `"${label}" names dependency "${depName}" — not found in any package.json / ${CARGO_FILE} on disk. ` +
          `Update the entry's dependencyName to the current package name, or — if it was genuinely dropped from the ` +
          `stack — move the entry to ring: 'hold' with subjectKind: 'not-adopted' (or 'technique'/'service' if that ` +
          `fits better), or delete the entry entirely.`
      );
    }
  }

  if (entry.adrSlug && !exists(join(ADR_DIR, `${entry.adrSlug}.md`))) {
    errors.push(
      `"${label}" links adrSlug "${entry.adrSlug}" — no such file at ${ADR_DIR}/${entry.adrSlug}.md. ` +
        `Fix the slug, or remove adrSlug if the ADR was renamed/removed.`
    );
  }
}

if (errors.length > 0) {
  console.error(`check:tech-radar FAILED — ${errors.length} issue(s) in ${RADAR_FILE}:\n`);
  for (const e of errors) console.error(`  - ${e}`);
  console.error('\nFix the entries above, then re-run.');
  process.exit(1);
}

console.log(
  `check:tech-radar OK — ${entries.length} entries checked against ${knownDeps.size} known dependency names; every adrSlug resolves.`
);
