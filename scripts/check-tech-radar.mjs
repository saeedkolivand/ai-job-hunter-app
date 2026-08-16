// Staleness guard for apps/landing/src/data/tech-radar.ts (the /tech-radar
// page — a curated, human-judged Adopt/Trial/Assess/Hold list, NOT derived
// from package.json: the whole point of a radar is the judgment call, and
// this repo already learned the hard way (four stale hardcoded AI-model
// defects in one session, see docs/knowledge/decision-records/0022) that a curated list left
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
// Checks:
//   1. Every `subjectKind: 'dependency'` entry's dependencyName (or `name` if
//      omitted) must be a real dependency key in a package.json / Cargo.toml
//      on disk today.
//   2. Where `name` states a bare major version (e.g. "Tauri 2"), that major
//      must match the major the manifest's declared range resolves to — the
//      check above only proves the PACKAGE exists, not that the claimed
//      VERSION is still current.
//   3. Every `adrSlug` must name a real file under docs/knowledge/decision-records/.
//   4. Every `id` is unique (it's a React key, a DOM anchor id, and a
//      layoutBlips() map key — a duplicate silently collides all three).
//   5. Every `lastReviewed` matches YYYY-MM-DD — the field this whole design
//      rests on, and it renders verbatim into public copy. No max-age rule
//      here on purpose: that is a judgement call, and a check that cries
//      wolf on an entry someone deliberately re-affirmed gets ignored.
//   6. A raw count of `id:` lines inside the RADAR array must equal the
//      number of entries the parser actually extracted — guards the parser
//      itself: an entry whose first key isn't `id`, or one written on a
//      single line, would otherwise be silently skipped rather than
//      checked, which defeats every check above without ever failing.
//
// Read-only. Wired as `pnpm check:tech-radar`; runs in ci-pipeline.yml's
// Lint & Format job and in the pre-push hook.

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const RADAR_FILE = 'apps/landing/src/data/tech-radar.ts';
const ADR_DIR = 'docs/knowledge/decision-records';
const CARGO_FILE = 'apps/desktop/src-tauri/Cargo.toml';

const read = (rel) => readFileSync(join(ROOT, rel), 'utf8');
const exists = (rel) => existsSync(join(ROOT, rel));

// ── A string/comment-aware bracket walk ─────────────────────────────────────
// Shared by both entry-block extraction ({}) and RADAR-array bounding ([]):
// starting at `source[openIndex]` (must be the opening bracket), returns the
// index one past its matching close, treating `'`, `"`, AND `` ` `` as
// string delimiters (each closed only by its OWN matching character, with
// `\`-escaping inside) and skipping `//` line comments — so neither an
// apostrophe/backtick in prose nor a `//` comment can desynchronize the
// depth count and swallow real content past it (the exact failure mode a
// naive "no braces/comments in strings" convention hits the day an entry's
// rationale uses a contraction in a template literal, or anyone adds an
// inline comment).
function scanBalanced(source, openIndex) {
  const open = source[openIndex];
  const close = open === '{' ? '}' : ']';
  let depth = 0;
  let inString = null; // the quote/backtick char currently open, or null
  let i = openIndex;
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
    if (ch === "'" || ch === '"' || ch === '`') {
      inString = ch;
      continue;
    }
    if (ch === '/' && source[i + 1] === '/') {
      const nl = source.indexOf('\n', i);
      i = nl === -1 ? source.length : nl; // the for-loop's i++ lands past it
      continue;
    }
    if (ch === open) depth++;
    else if (ch === close) {
      depth--;
      if (depth === 0) return i + 1;
    }
  }
  return i;
}

/** Every `{ id: '...', ... }`-shaped top-level block in `source`, verbatim. */
function extractObjectBlocks(source) {
  const startRe = /\{\s*\n\s*id:\s*'/g;
  const blocks = [];
  let m;
  while ((m = startRe.exec(source))) {
    const end = scanBalanced(source, m.index);
    blocks.push(source.slice(m.index, end));
    startRe.lastIndex = end;
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
      lastReviewed: field(block, 'lastReviewed'),
    }));
}

// Raw `id:` line count within the RADAR array's own bounds ONLY — NOT the
// whole file, which would also count QUADRANTS/RINGS entries and (harmlessly
// but confusingly) the `id: string;` line in the TechRadarEntry interface
// (that one has no quote after the colon, so it wouldn't match anyway, but
// scoping to the array is what makes this an honest apples-to-apples count).
function countRawIdLines(source) {
  const marker = source.indexOf('export const RADAR');
  if (marker === -1) return null;
  // NOT indexOf('[', marker) — the type annotation itself contains one
  // (`TechRadarEntry[]`), which closes immediately and would bound an
  // effectively empty "array" before the real literal is ever reached.
  // '= [' is the assignment's actual array-literal opener.
  const assign = source.indexOf('= [', marker);
  if (assign === -1) return null;
  const open = assign + 2;
  const close = scanBalanced(source, open);
  const body = source.slice(open, close);
  return (body.match(/^\s*id:\s*['"]/gm) ?? []).length;
}

// ── Collect every dependency name (+ declared version range) in the repo ────
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

/** name -> [{ file, range }, ...] — every declaration site, not just the first. */
function collectPackageJsonDeps(declarations) {
  for (const file of packageJsonFiles()) {
    const json = JSON.parse(read(file));
    for (const fieldName of PKG_JSON_FIELDS) {
      for (const [name, range] of Object.entries(json[fieldName] ?? {})) {
        if (!declarations.has(name)) declarations.set(name, []);
        declarations.get(name).push({ file, range });
      }
    }
  }
}

// Minimal Cargo.toml dependency-table scanner: tracks the current `[section]`
// header and records a bare `name = ...` key (plus its declared version,
// when written as `name = "x"` or `name = { version = "x", ... }` on that
// same line — every entry in this file's [dependencies] does that) while
// inside any section whose path ends in "dependencies" (covers
// [dependencies], [dev-dependencies], [build-dependencies], and every
// per-target [target.'cfg(...)'.dependencies] table) — comment lines are
// skipped first so a `#`-prefixed line can never be misread as one.
function extractCargoDeps(text) {
  const declarations = new Map(); // name -> [{ file: CARGO_FILE, range }]
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
    const m = line.match(/^"?([A-Za-z0-9_.-]+)"?\s*=\s*(.+)$/);
    if (!m) continue;
    const [, name, rest] = m;
    const bare = rest.match(/^"([^"]+)"/); // name = "1.2.3"
    const tabled = rest.match(/version\s*=\s*"([^"]+)"/); // name = { version = "1.2.3", ... }
    const range = bare?.[1] ?? tabled?.[1] ?? null;
    if (!declarations.has(name)) declarations.set(name, []);
    declarations.get(name).push({ file: CARGO_FILE, range });
  }
  return declarations;
}

function collectDependencyDeclarations() {
  const declarations = new Map();
  collectPackageJsonDeps(declarations);
  if (exists(CARGO_FILE)) {
    for (const [name, sites] of extractCargoDeps(read(CARGO_FILE))) {
      if (!declarations.has(name)) declarations.set(name, []);
      declarations.get(name).push(...sites);
    }
  }
  return declarations;
}

// A leading semver-ish operator (^, ~, >=, <=, >, <, =) or none (Cargo's bare
// "2" and npm's exact "19.2.8" both mean "no operator"), then the first
// dotted numeric component. Deliberately permissive about what follows —
// this only ever reads the MAJOR. Returns null (unparseable, never fails the
// check) for workspace:*, git/URL specs, "*", "latest"/tags, and anything
// else with no leading digit after stripping the operator.
function parseMajor(range) {
  if (!range) return null;
  const stripped = range.trim().replace(/^(\^|~|>=|<=|>|<|=)\s*/, '');
  const m = stripped.match(/^(\d+)/);
  return m ? Number(m[1]) : null;
}

// The version an entry's `name` states, IF it states one: a bare integer at
// the very end (optionally preceded by whitespace), so "Tauri 2" -> 2 but
// "Next.js (static export)", "WCAG 2.2 AA", and "docx-rs" all -> null (no
// trailing standalone integer) — deliberately not matching a
// decimal/multi-part version: entries are asked to keep `name` to at most a
// major version and put finer detail (e.g. "React 19.2") in `rationale`,
// which this check does not parse or verify (it's dated by `lastReviewed`,
// not asserted as current).
function claimedMajor(name) {
  if (!name) return null;
  const m = name.match(/(?:^|\s)(\d+)\s*$/);
  return m ? Number(m[1]) : null;
}

// ── Run ───────────────────────────────────────────────────────────────────
if (!exists(RADAR_FILE)) {
  console.error(`check:tech-radar FAILED — ${RADAR_FILE} not found.`);
  process.exit(1);
}

const source = read(RADAR_FILE);
const entries = parseRadarEntries(source);
const rawIdCount = countRawIdLines(source);

// Ordered so the MOST SPECIFIC diagnosis wins: a structurally-missing array
// is a different problem than a count mismatch (checked next, and catches
// "every single entry got skipped" — entries.length === 0 with rawIdCount >
// 0 — just as well as a partial one, so there's no separate "zero parsed"
// special case to accidentally shadow it), which is itself different from a
// RADAR array that's genuinely empty (both counts legitimately zero).
if (rawIdCount === null) {
  console.error(
    `check:tech-radar FAILED — couldn't locate 'export const RADAR = [...]' in ${RADAR_FILE} at all. The parser's array-bounding logic (countRawIdLines in this script) no longer matches the data file's formatting — fix whichever one drifted.`
  );
  process.exit(1);
}

if (rawIdCount !== entries.length) {
  console.error(
    `check:tech-radar FAILED — the RADAR array has ${rawIdCount} 'id:' line(s) but the parser only extracted ${entries.length} entrie(s). ` +
      `This means at least one entry was silently skipped — likely because its FIRST key isn't 'id', it's written on one line, or 'subjectKind' shares ` +
      `a line with another field (extractObjectBlocks/field() in this script both assume Prettier's one-key-per-line formatting). ` +
      `Run 'pnpm format' and re-run this check before assuming the entries themselves are wrong.`
  );
  process.exit(1);
}

if (entries.length === 0) {
  console.error(`check:tech-radar FAILED — the RADAR array in ${RADAR_FILE} is empty.`);
  process.exit(1);
}

const declarations = collectDependencyDeclarations();
const errors = [];
const seenIds = new Map(); // id -> first entry's label, for a readable dupe report

for (const entry of entries) {
  const label = entry.name ?? entry.id ?? '(unnamed entry)';

  if (!entry.id) {
    errors.push(`"${label}" has no 'id' field.`);
  } else if (seenIds.has(entry.id)) {
    errors.push(
      `duplicate id "${entry.id}" — used by both "${seenIds.get(entry.id)}" and "${label}". ` +
        `'id' is a React key, the DOM anchor tr-entry-<id>, and a layoutBlips() map key — a duplicate silently collides all three (two blips stack on top of each other). Rename one.`
    );
  } else {
    seenIds.set(entry.id, label);
  }

  if (!entry.lastReviewed || !/^\d{4}-\d{2}-\d{2}$/.test(entry.lastReviewed)) {
    errors.push(
      `"${label}" has lastReviewed: ${entry.lastReviewed ? `"${entry.lastReviewed}"` : '(missing)'} — must be YYYY-MM-DD. It renders verbatim on the public page.`
    );
  }

  if (entry.subjectKind === 'dependency') {
    const depName = entry.dependencyName ?? entry.name;
    const sites = depName ? declarations.get(depName) : undefined;
    if (!depName) {
      errors.push(
        `"${label}" has subjectKind: 'dependency' but no dependencyName (and no name) to check.`
      );
    } else if (!sites) {
      errors.push(
        `"${label}" names dependency "${depName}" — not found in any package.json / ${CARGO_FILE} on disk. ` +
          `Update the entry's dependencyName to the current package name, or — if it was genuinely dropped from the ` +
          `stack — move the entry to ring: 'hold' with subjectKind: 'not-adopted' (or 'technique'/'service' if that ` +
          `fits better), or delete the entry entirely.`
      );
    } else {
      const claimed = claimedMajor(entry.name);
      if (claimed !== null) {
        for (const { file, range } of sites) {
          const declaredMajor = parseMajor(range);
          if (declaredMajor !== null && declaredMajor !== claimed) {
            errors.push(
              `"${label}" claims major version ${claimed}, but ${file} declares "${depName}": "${range}" (major ${declaredMajor}). ` +
                `Update the number in the entry's name to match, or drop it from name entirely and state the version in rationale instead (dated by lastReviewed).`
            );
          }
        }
      }
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
  `check:tech-radar OK — ${entries.length} entries checked (all ids unique, all lastReviewed dates well-formed) against ${declarations.size} known dependency names; every claimed major version and every adrSlug resolves.`
);
