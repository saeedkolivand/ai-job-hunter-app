/**
 * bump-last-updated — auto-update markdown "Last updated: YYYY-MM-DD" headers.
 *
 * Given file paths as argv, for each `.md` file whose first ~10 lines contain
 * a "Last updated: YYYY-MM-DD" line, replaces the date with today (or git log
 * date if --backfill-from-git). Preserves trailing text like "(task #16: ...)".
 * Idempotent: only writes when the date actually changed.
 *
 * Usage:
 *   node scripts/bump-last-updated.mjs path/to/file.md path/to/other.md
 *   node scripts/bump-last-updated.mjs --backfill-from-git path/to/file.md
 *
 * Wire into lint-staged.config.mjs after the markdown pattern:
 *   node scripts/bump-last-updated.mjs BEFORE prettier --write
 *
 * The backfill flag reads git log dates for historical corrections (one-shot).
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

/**
 * Get the bump date from environment or fall back to today.
 * Validates BUMP_DATE env var matches YYYY-MM-DD; ignores invalid values with warning.
 */
function getBumpDate() {
  const envDate = process.env.BUMP_DATE;
  if (envDate) {
    if (/^\d{4}-\d{2}-\d{2}$/.test(envDate)) {
      return envDate;
    }
    process.stderr.write(
      `Warning: BUMP_DATE="${envDate}" does not match YYYY-MM-DD format; using today's date.\n`
    );
  }
  return getTodayDate();
}

/**
 * Get today's date in YYYY-MM-DD format.
 */
function getTodayDate() {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

/**
 * Get the last git change date for a file in YYYY-MM-DD format.
 * Returns undefined if the file is untracked or git fails.
 */
function getGitChangeDate(filePath) {
  try {
    const output = execFileSync(
      'git',
      ['log', '-1', '--date=short', '--format=%ad', '--', filePath],
      {
        encoding: 'utf-8',
        stdio: ['pipe', 'pipe', 'ignore'], // suppress stderr
      }
    ).trim();
    return output || undefined;
  } catch {
    return undefined;
  }
}

/**
 * Bump the "Last updated: YYYY-MM-DD" header in a markdown file.
 * Only touches the date; preserves trailing text like "(v1.0)" or "(task #16: ...)".
 * Preserves exact line endings in the file (doesn't normalize).
 * Returns true if the file was modified, false otherwise.
 */
function bumpLastUpdated(filePath, newDate) {
  const content = readFileSync(filePath, 'utf-8');

  // Split by any line ending (LF or CRLF), keeping track of original line ending style.
  // Use a regex that captures the line ending so we can restore it exactly.
  const lineEndingMatch = content.match(/\r\n|\n/);
  const lineEnding = lineEndingMatch ? lineEndingMatch[0] : '\n';
  const lines = content.split(lineEndingMatch ? lineEndingMatch[0] : /\r\n|\n/);

  // Look for the "Last updated:" line in the first ~10 lines
  let headerIndex = -1;
  let oldDate = null;

  for (let i = 0; i < Math.min(10, lines.length); i++) {
    const match = lines[i].match(/^Last updated: (\d{4}-\d{2}-\d{2})(.*)$/);
    if (match) {
      headerIndex = i;
      oldDate = match[1];
      break;
    }
  }

  if (headerIndex === -1) {
    // No "Last updated:" header found, skip this file
    return false;
  }

  if (oldDate === newDate) {
    // Date is already current, no change needed
    return false;
  }

  // Replace only the header line, keeping the date that was captured
  lines[headerIndex] = `Last updated: ${newDate}${lines[headerIndex].substring(
    `Last updated: ${oldDate}`.length
  )}`;

  const modified = lines.join(lineEnding);

  // Only write if content actually changed
  if (modified !== content) {
    writeFileSync(filePath, modified, 'utf-8');
    return true;
  }

  return false;
}

/**
 * Main: process argv as file paths.
 */
function main() {
  const argv = process.argv.slice(2);
  const backfillFromGit = argv.includes('--backfill-from-git');

  const files = argv.filter((arg) => !arg.startsWith('--'));

  if (files.length === 0) {
    // No files provided, exit silently (common when no .md files are staged)
    process.exit(0);
  }

  let modified = 0;
  let skipped = 0;

  for (const filePath of files) {
    if (!filePath.endsWith('.md')) {
      skipped++;
      continue;
    }

    let newDate;
    if (backfillFromGit) {
      newDate = getGitChangeDate(filePath);
      if (!newDate) {
        // Git date unavailable (untracked file, etc.); use bump date (env or today)
        newDate = getBumpDate();
      }
    } else {
      newDate = getBumpDate();
    }

    if (bumpLastUpdated(filePath, newDate)) {
      modified++;
      process.stderr.write(`✓ ${filePath} → ${newDate}\n`);
    }
  }

  if (modified > 0 || backfillFromGit) {
    process.stderr.write(`Last updated bumped: ${modified} file(s), ${skipped} non-.md skipped.\n`);
  }
}

try {
  main();
} catch (err) {
  process.stderr.write(`Error: ${err.message}\n`);
  process.exit(1);
}
