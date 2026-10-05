// Drift guard: every git-tracked *.ts / *.tsx file stays at or under HARD_CAP
// code lines (tests included, *.d.ts excluded) — the TypeScript twin of the
// Rust guard `r8_no_oversized_modules` in apps/desktop/src-tauri/tests/
// architecture.rs, with the same semantics, baseline format and bless flow.
//
// ── What counts ──────────────────────────────────────────────────────────────
//
// A line counts when, after removing comments, it still has non-whitespace
// text — ESLint's `max-lines` with `{ skipBlankLines: true, skipComments:
// true }`. Comment ranges come from a real parser (`@typescript-eslint/
// parser`), never a regex: JSX text like `<p>Don't</p>` and strings or
// template literals containing `//` or `/*` would skew a scanner. A file that
// fails to parse is an error, not a count of 0.
//
// ── The ratchet ──────────────────────────────────────────────────────────────
//
// Files already over the cap are recorded in scripts/data/ts-size-baseline.txt
// (`<loc>\t<path>`): a baselined file may shrink or be deleted, never grow, and
// a new file over the cap is not baselined — it fails until it is split. An
// entry whose file now fits (or is gone) is stale and fails too, so the list
// cannot rot. After a split, regenerate with:
//
//   TS_SIZE_BLESS=1 node scripts/check-ts-size.mjs

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { parse } from '@typescript-eslint/parser';

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');

/** Max code lines per file — the one place the cap lives. */
export const HARD_CAP = 300;

export const BASELINE_REL = 'scripts/data/ts-size-baseline.txt';

export const BASELINE_HEADER = `\
# TS size baseline — one line per tracked *.ts/*.tsx file over HARD_CAP: \`<loc>\t<path>\`, \`path\` = repo-relative POSIX path.
# \`<loc>\` is CODE lines (blank and comment lines excluded).
# A baselined file may shrink or be deleted, never grow; a new over-cap file is not baselined.
# Regenerate after a split with: TS_SIZE_BLESS=1 node scripts/check-ts-size.mjs
`;

/**
 * Code lines in `source`: lines that still have non-whitespace text once every
 * comment is blanked out. `filePath` only selects the dialect (`.tsx` parses JSX).
 * Throws if the source does not parse.
 */
export function countCodeLines(source, filePath) {
  const { comments } = parse(source, {
    comment: true,
    range: true,
    jsx: filePath.endsWith('.tsx'),
  });
  let code = '';
  let at = 0;
  for (const { range } of comments) {
    // Blank the comment's characters but keep its newlines, so line numbering holds.
    code += source.slice(at, range[0]) + source.slice(range[0], range[1]).replace(/[^\r\n]/g, ' ');
    at = range[1];
  }
  code += source.slice(at);
  return code.split('\n').filter((line) => line.trim() !== '').length;
}

/**
 * Violations of the cap + ratchet, as `path: message` lines sorted by path.
 * Empty means the invariant holds. `counts` maps every tracked file to its code
 * lines; `baseline` maps each baselined file to its recorded count.
 */
export function evaluate({ counts, baseline }) {
  const found = [];
  for (const [path, loc] of Object.entries(counts)) {
    if (loc <= HARD_CAP) continue;
    const recorded = baseline[path];
    if (recorded === undefined) {
      found.push([path, `${loc} LOC — new file over the ${HARD_CAP}-line cap, split it`]);
    } else if (loc > recorded) {
      found.push([path, `${loc} LOC — grew past its baseline of ${recorded}`]);
    }
  }
  for (const path of Object.keys(baseline)) {
    if (counts[path] > HARD_CAP) continue;
    const why = path in counts ? 'now fits' : 'no longer exists';
    found.push([path, `${why} — remove it from ${BASELINE_REL}`]);
  }
  return found.sort(([a], [b]) => (a < b ? -1 : 1)).map(([path, message]) => `${path}: ${message}`);
}

/** Parse baseline text into `path -> loc`; a malformed line throws, naming it. */
export function parseBaseline(text) {
  const baseline = {};
  text.split('\n').forEach((raw, i) => {
    const line = raw.trim();
    if (line === '' || line.startsWith('#')) return;
    const match = /^(\d+)\t(.+)$/.exec(line);
    if (!match || match[2] in baseline) {
      throw new Error(`${BASELINE_REL}:${i + 1}: malformed or duplicate line "${line}"`);
    }
    baseline[match[2]] = Number(match[1]);
  });
  return baseline;
}

/** Baseline text for `counts`: the header, then the over-cap files sorted by path. */
export function formatBaseline(counts) {
  const rows = Object.entries(counts)
    .filter(([, loc]) => loc > HARD_CAP)
    .sort(([a], [b]) => (a < b ? -1 : 1))
    .map(([path, loc]) => `${loc}\t${path}\n`);
  return BASELINE_HEADER + rows.join('');
}

/** Repo-relative POSIX paths of every tracked, still-present *.ts / *.tsx (minus *.d.ts). */
function trackedSources() {
  const out = execFileSync('git', ['ls-files', '-z', '--', '*.ts', '*.tsx'], {
    cwd: REPO_ROOT,
    encoding: 'utf8',
  });
  return out
    .split('\0')
    .filter((path) => path && !path.endsWith('.d.ts') && existsSync(resolve(REPO_ROOT, path)));
}

/** Runs the check (or the bless); returns the process exit code. */
function main() {
  const counts = {};
  const unparsable = [];
  for (const path of trackedSources()) {
    try {
      counts[path] = countCodeLines(readFileSync(resolve(REPO_ROOT, path), 'utf8'), path);
    } catch (e) {
      unparsable.push(`${path}: failed to parse — ${e.message}`);
    }
  }
  if (unparsable.length > 0) {
    for (const line of unparsable) console.error(line);
    console.error(`✗ check:ts-size — ${unparsable.length} file(s) failed to parse.`);
    return 1;
  }

  const baselineFile = resolve(REPO_ROOT, BASELINE_REL);
  if (process.env.TS_SIZE_BLESS === '1') {
    writeFileSync(baselineFile, formatBaseline(counts));
    const over = Object.values(counts).filter((loc) => loc > HARD_CAP).length;
    console.log(`check:ts-size — blessed ${over} over-cap file(s) into ${BASELINE_REL}.`);
    return 0;
  }

  if (!existsSync(baselineFile)) {
    console.error(`✗ ${BASELINE_REL} is missing — restore it, or re-run with TS_SIZE_BLESS=1.`);
    return 1;
  }
  const problems = evaluate({
    counts,
    baseline: parseBaseline(readFileSync(baselineFile, 'utf8')),
  });
  if (problems.length > 0) {
    for (const line of problems) console.error(line);
    console.error(
      `✗ check:ts-size FAILED — ${problems.length} file(s) over the ${HARD_CAP}-line cap or ` +
        `with a stale baseline entry. Split the file (a baselined one may only shrink); after ` +
        `a split regenerate with: TS_SIZE_BLESS=1 node scripts/check-ts-size.mjs`
    );
    return 1;
  }
  const over = Object.values(counts).filter((loc) => loc > HARD_CAP).length;
  console.log(
    `check:ts-size OK — ${Object.keys(counts).length} file(s) scanned, ${over} over the ` +
      `${HARD_CAP}-line cap, all baselined and none grown.`
  );
  return 0;
}

// Skipped when imported by the test file.
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = main();
  } catch (e) {
    // Message only: a stack trace would print absolute paths (AGENTS.md path privacy).
    console.error(`✗ check:ts-size — ${e.message}`);
    process.exitCode = 1;
  }
}
