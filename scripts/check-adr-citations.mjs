// Drift guard: every ADR citation in the repo resolves to a real ADR.
//
// ── The defect this exists to stop ──────────────────────────────────────────
//
// This repo has ONE ADR directory holding TWO numbering series. The knowledge
// base's own `adr-NNN-` series is open and runs to the thirties; a second tree
// grew under `docs/adr/` and was folded in by #1000 as the `NNNN-` series,
// deliberately NOT renumbered because an ADR is a dated record whose number is
// cited from commit messages, merged PR bodies, code comments and generated
// landing snapshots that must not be rewritten.
//
// The consequence is that every number from 1 to 23 exists TWICE, and the only
// thing telling the two apart is the zero padding:
//
//   ADR-0013  → 0013-email-confirmation-watching.md          (closed series)
//   ADR-013   → adr-013-resume-builder-base-plus-handoff.md  (open series)
//
// That convention is load-bearing across ~900 citations and was, until now,
// enforced by nothing. Two citations had already drifted off it by the time
// this check was written, and both were found by hand:
//
//   * ADR-0020 linked the diagnostics-bundle ADR correctly but LABELLED it with
//     the FOUR-digit form of 27, which the closed series does not have. The
//     prose and the link disagreed, and a reader following the prose would go
//     looking for a record that does not exist.
//   * ARCHITECTURE_STATUS attributed the Tauri-coupling burndown to the
//     four-digit form of 25 — written a month before ADR-025 existed, and about
//     an unrelated subject. Re-padding it would have pointed at the agent-fleet
//     ADR: a confident citation of the wrong decision, which is worse than a
//     dangling one.
//
// (Those two are written in words rather than as citations because this file is
// scanned like any other, and the guard correctly rejects both. Nothing here is
// exempt from it.)
//
// A mis-padded number is unrecoverable after the fact: nothing can tell which
// of the two records the author meant. So the only useful moment to catch it is
// the moment it is written, which is what this check does.
//
// ── What this enforces ──────────────────────────────────────────────────────
//
//   1. Every `decision-records/<file>.md` path reference resolves on disk.
//   2. Every `ADR-NNN` / `ADR NNN` citation resolves under the padding rule:
//      four digits mean the closed series, three mean the open one.
//   3. No citation is written with one or two digits, because that is
//      ambiguous between the two series and no tool can recover the intent.
//
// The ADR inventory is DISCOVERED from the directory, not listed here, so a new
// ADR is covered the day it is written.
//
// Generated landing snapshots under `apps/landing/public/` are excluded: they
// are frozen captures of historical commit and PR text, records rather than
// citations, and rewriting them would falsify the history they exist to hold.

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const ADR_DIR = 'docs/knowledge/decision-records';

/**
 * Files excluded from the scan, and why. Kept deliberately tiny — every entry
 * is a hole, so each one has to earn its place.
 */
const EXCLUDED = [
  // Frozen captures of commit/PR text. Records of what was written, not live
  // citations, and they legitimately contain the retired `docs/adr/` paths.
  /^apps\/landing\/public\//,
  // This check's own test fixtures deliberately contain broken citations
  // (a mis-padded number, an absent one, an ambiguous one). Scanning them
  // would make the guard fail on the tests that prove it works.
  /^scripts\/check-adr-citations\.test\.mjs$/,
];

/** Extensions that are never worth reading as text. */
const BINARY =
  /\.(png|jpg|jpeg|gif|webp|avif|ico|icns|woff2?|ttf|otf|eot|pdf|zip|gz|mp4|webm|wasm|node|dll|exe|so|dylib|docx?)$/i;

// ── Vacuity thresholds ──────────────────────────────────────────────────────
//
// Every check below is "this list of problems is empty", which is exactly the
// shape that passes trivially when discovery breaks. If `git ls-files` returns
// nothing, or the citation regex stops matching, the guard goes green while
// checking nothing at all. These floors turn that silence into a failure.
//
// At the time of writing the repo scans 2285 files and finds 911 citations,
// 143 path references and 57 ADRs.
const MIN_SCANNED_FILES = 500;
const MIN_ADR_FILES = 40;
/**
 * Citations are floored PER FORM, because the failure being guarded is a form
 * dying — the pattern quietly ceasing to match `ADR NNN` while `ADR-NNN` keeps
 * working. A single total can only ever see that diluted by the forms that
 * survived: the spaced form is 143 of 911 today, so losing all of it is a ~16%
 * dip that any total-only floor loose enough to be safe would sail past. Worse,
 * a total tuned to catch it today silently stops meaning that as the mix
 * shifts — a batch of new ADRs cited mostly with hyphens would restore the
 * headroom and reopen the hole with no diff anywhere.
 *
 * Per-form floors do not have that property: dropping either form fails on its
 * own floor no matter what the other does. The total stays as a cheap backstop
 * for a collapse that somehow spares both forms proportionally.
 *
 * Current counts: 768 hyphen, 143 spaced, 911 total.
 *
 * So if one of these fails, suspect the pattern FIRST. Drifting down through
 * them means someone removed hundreds of references — confirm that was
 * deliberate before lowering a number.
 */
const MIN_HYPHEN_CITATIONS = 600;
const MIN_SPACED_CITATIONS = 100;
const MIN_CITATIONS = 800;
const MIN_PATH_REFS = 50;

/**
 * Split ADR filenames into the two series.
 *
 * `closed` is keyed by the four-digit number, `open` by the three-digit one —
 * the same keys a citation is classified into, so resolution is a map lookup
 * rather than a second, separately-drifting pattern.
 */
export function classifyAdrFiles(filenames) {
  const closed = new Map();
  const open = new Map();
  const unclassified = [];
  for (const name of filenames) {
    if (!name.endsWith('.md')) continue;
    const closedMatch = /^(\d{4})-/.exec(name);
    const openMatch = /^adr-(\d{3})-/.exec(name);
    if (closedMatch) closed.set(closedMatch[1], name);
    else if (openMatch) open.set(openMatch[1], name);
    else unclassified.push(name);
  }
  return { closed, open, unclassified };
}

/** ADR filenames on disk. */
export function adrFilenames(dir = join(REPO_ROOT, ADR_DIR)) {
  return existsSync(dir) ? readdirSync(dir).filter((f) => f.endsWith('.md')) : [];
}

/** Tracked, scannable files as repo-relative POSIX paths. */
export function trackedFiles(root = REPO_ROOT) {
  const out = execFileSync('git', ['ls-files', '-z'], {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 1 << 28,
  });
  return out
    .split('\0')
    .filter(Boolean)
    .filter((p) => !BINARY.test(p))
    .filter((p) => !EXCLUDED.some((re) => re.test(p)));
}

/** 1-based line number of `index` within `text`. */
const lineOf = (text, index) => text.slice(0, index).split('\n').length;

/**
 * Scan one file's text for ADR references.
 *
 * Returns counts alongside problems: the counts are what the vacuity guards
 * read, and a check that cannot say how much it looked at cannot prove it
 * looked at anything.
 */
export function scanText(path, text, inv) {
  const badPaths = [];
  const badCites = [];
  const ambiguous = [];
  let pathRefs = 0;
  let citations = 0;
  const byForm = { hyphen: 0, spaced: 0 };

  const known = new Set([...inv.closed.values(), ...inv.open.values(), ...inv.unclassified]);

  // 1. Explicit path references into the ADR directory.
  for (const m of text.matchAll(/(?:docs\/knowledge\/)?decision-records\/([\w.-]+?\.md)/g)) {
    // Prose elision, e.g. "adr-007-...md" in an audit narrative, describes a
    // filename rather than linking to one.
    if (m[1].includes('...')) continue;
    pathRefs++;
    if (!known.has(m[1])) badPaths.push(`${path}:${lineOf(text, m.index)} → ${m[0]}`);
  }

  // 2. Number citations, in every form the repo uses: hyphen or space, upper
  //    or lower case. `\d+` rather than `\d{3,4}` so an over- or under-padded
  //    number is REPORTED instead of silently failing to match — a citation
  //    the pattern skips is exactly the one nobody ever finds.
  //
  //    The SEPARATOR is captured rather than left inside a character class, so
  //    both forms are counted from this one pass. Counting them with a second
  //    pair of regexes would let the per-form floors guard a different
  //    population than the resolver actually checks, and the two could then
  //    disagree about what they matched without anything noticing.
  for (const m of text.matchAll(/\bADR([- ])(\d+)\b/gi)) {
    const n = m[2];
    citations++;
    if (m[1] === '-') byForm.hyphen++;
    else byForm.spaced++;
    const where = `${path}:${lineOf(text, m.index)} → ${m[0]}`;
    if (n.length === 4) {
      if (!inv.closed.has(n)) badCites.push(`${where} — no ${ADR_DIR}/${n}-*.md (closed series)`);
    } else if (n.length === 3) {
      if (!inv.open.has(n)) badCites.push(`${where} — no ${ADR_DIR}/adr-${n}-*.md (open series)`);
    } else if (n.length < 3) {
      ambiguous.push(
        `${where} — ${n.length} digit(s): pad to 4 for the closed series ` +
          `(ADR-${n.padStart(4, '0')}) or 3 for the open one (ADR-${n.padStart(3, '0')})`
      );
    } else {
      badCites.push(`${where} — ${n.length} digits matches neither series`);
    }
  }

  return { pathRefs, citations, byForm, badPaths, badCites, ambiguous };
}

/** Aggregate `scanText` over `[{ path, text }]`. */
export function scan(files, inv) {
  const total = {
    pathRefs: 0,
    citations: 0,
    byForm: { hyphen: 0, spaced: 0 },
    badPaths: [],
    badCites: [],
    ambiguous: [],
  };
  for (const { path, text } of files) {
    // Cheap pre-filter: most of the repo never says "adr" at all.
    if (!/adr/i.test(text)) continue;
    const r = scanText(path, text, inv);
    total.pathRefs += r.pathRefs;
    total.citations += r.citations;
    total.byForm.hyphen += r.byForm.hyphen;
    total.byForm.spaced += r.byForm.spaced;
    total.badPaths.push(...r.badPaths);
    total.badCites.push(...r.badCites);
    total.ambiguous.push(...r.ambiguous);
  }
  return total;
}

/** Read the scannable files, skipping anything unreadable as text. */
export function readFiles(paths, root = REPO_ROOT) {
  const out = [];
  for (const path of paths) {
    const full = join(root, path);
    try {
      if (!statSync(full).isFile()) continue;
      out.push({ path, text: readFileSync(full, 'utf8') });
    } catch {
      // Unreadable (submodule, broken link, permissions) — not a citation
      // problem, and failing here would make the guard about the filesystem.
    }
  }
  return out;
}

/**
 * Every violation, as human-readable lines. Empty means the invariant holds.
 *
 * Returned rather than printed so the check is testable without capturing
 * stdout or trapping `process.exit`.
 */
export function violations(stats, inv, scannedFiles) {
  const problems = [];

  // ── Vacuity guards ────────────────────────────────────────────────────────
  // These run FIRST and return early. Everything after them is an emptiness
  // assertion, and an emptiness assertion over an empty scan is theatre — so
  // the "did we actually look at anything" question has to be answered before
  // its answer can be trusted.
  if (scannedFiles < MIN_SCANNED_FILES) {
    problems.push(
      `Only ${scannedFiles} files scanned (expected at least ${MIN_SCANNED_FILES}) — the file ` +
        'walk is broken, so every check below passes without reading the repo. Fix discovery ' +
        'before trusting a green run.'
    );
    return problems;
  }
  const adrCount = inv.closed.size + inv.open.size;
  if (adrCount < MIN_ADR_FILES) {
    problems.push(
      `Only ${adrCount} ADRs found in ${ADR_DIR}/ (expected at least ${MIN_ADR_FILES}) — the ` +
        'filename patterns no longer match the records on disk, so citations are being ' +
        'resolved against an inventory that is nearly empty.'
    );
    return problems;
  }
  // The count floors are COLLECTED rather than returned one at a time. When a
  // single form dies, its own floor and the total backstop both trip, and
  // seeing both together is what distinguishes "one form stopped matching"
  // from "everything did" — reporting only the first would hide that.
  const counts = [];
  if (stats.byForm.hyphen < MIN_HYPHEN_CITATIONS) {
    counts.push(
      `hyphen form \`ADR-NNN\`: ${stats.byForm.hyphen} found, floor ${MIN_HYPHEN_CITATIONS}`
    );
  }
  if (stats.byForm.spaced < MIN_SPACED_CITATIONS) {
    counts.push(
      `spaced form \`ADR NNN\`: ${stats.byForm.spaced} found, floor ${MIN_SPACED_CITATIONS}`
    );
  }
  if (stats.citations < MIN_CITATIONS) {
    counts.push(`total citations (backstop): ${stats.citations} found, floor ${MIN_CITATIONS}`);
  }
  if (stats.pathRefs < MIN_PATH_REFS) {
    counts.push(
      `decision-records path references: ${stats.pathRefs} found, floor ${MIN_PATH_REFS}`
    );
  }
  if (counts.length > 0) {
    problems.push(
      'Discovery is returning too little for a green run to mean anything — every check\n' +
        '  below is an emptiness assertion, and an emptiness assertion over an empty scan\n' +
        '  reports no problems no matter how many exist. Suspect the PATTERN first; a real\n' +
        '  drop this size means someone deleted hundreds of references on purpose:\n' +
        counts.map((c) => `    ${c}`).join('\n')
    );
    return problems;
  }

  if (inv.unclassified.length > 0) {
    problems.push(
      `These files in ${ADR_DIR}/ belong to neither series, so nothing can cite them by\n` +
        '  number. Name them `adr-NNN-slug.md` (the open series):\n' +
        inv.unclassified.map((f) => `    ${f}`).join('\n')
    );
  }

  if (stats.badPaths.length > 0) {
    problems.push(
      'These references point at an ADR file that does not exist:\n' +
        stats.badPaths.map((f) => `    ${f}`).join('\n') +
        '\n  Fix the path, or update it if the ADR was renamed.'
    );
  }

  if (stats.badCites.length > 0) {
    problems.push(
      'These ADR numbers resolve to nothing:\n' +
        stats.badCites.map((f) => `    ${f}`).join('\n') +
        '\n  Four digits mean the closed `NNNN-` series, three mean the open `adr-NNN` one.\n' +
        '  A number that resolves in neither is usually a padding slip — check WHICH decision\n' +
        '  you meant before repadding, because the same number exists in both series for\n' +
        '  everything up to 23 and the two are unrelated records.'
    );
  }

  if (stats.ambiguous.length > 0) {
    problems.push(
      'These citations are too short to identify a series:\n' +
        stats.ambiguous.map((f) => `    ${f}`).join('\n') +
        '\n  Every number up to 23 exists in BOTH series, so an unpadded number is ambiguous\n' +
        '  and no later reader can recover which record was meant.'
    );
  }

  return problems;
}

// Skipped when imported by the test file.
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const inv = classifyAdrFiles(adrFilenames());
  const files = readFiles(trackedFiles());
  const stats = scan(files, inv);
  const problems = violations(stats, inv, files.length);

  if (problems.length > 0) {
    for (const p of problems) console.error(`✗ ${p}`);
    process.exit(1);
  }

  console.log(
    `check:adr-citations OK — ${inv.closed.size + inv.open.size} ADRs ` +
      `(${inv.closed.size} closed \`NNNN-\`, ${inv.open.size} open \`adr-NNN\`); ` +
      `${stats.citations} number citations (${stats.byForm.hyphen} hyphen, ` +
      `${stats.byForm.spaced} spaced) and ${stats.pathRefs} path references across ` +
      `${files.length} tracked files all resolve, none ambiguous.`
  );
}

export {
  ADR_DIR,
  REPO_ROOT,
  MIN_SCANNED_FILES,
  MIN_ADR_FILES,
  MIN_HYPHEN_CITATIONS,
  MIN_SPACED_CITATIONS,
  MIN_CITATIONS,
  MIN_PATH_REFS,
};
