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
    const output = execFileSync('git', ['log', '-1', '--format=%as', '--', filePath], {
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'ignore'], // suppress stderr
    }).trim();
    return output || undefined;
  } catch {
    return undefined;
  }
}

/**
 * Bump the "Last updated: YYYY-MM-DD" header in a markdown file.
 * Only touches the date; preserves trailing text like "(v1.0)" or "(task #16: ...)".
 * Returns true if the file was modified, false otherwise.
 */
function bumpLastUpdated(filePath, newDate) {
  let content = readFileSync(filePath, 'utf-8');

  // Normalize line endings to \n for processing
  const normalized = content.replace(/\r\n/g, '\n');
  const lines = normalized.split('\n');

  // Look for the "Last updated:" line in the first ~10 lines
  let found = false;
  for (let i = 0; i < Math.min(10, lines.length); i++) {
    const line = lines[i];
    const match = line.match(/^(Last updated:) \d{4}-\d{2}-\d{2}(.*)$/);

    if (match) {
      const oldDate = line.match(/\d{4}-\d{2}-\d{2}/)[0];
      if (oldDate !== newDate) {
        // Replace the date, keep everything else (prefix + suffix)
        lines[i] = `${match[1]} ${newDate}${match[2]}`;
        found = true;
      } else {
        // Date is already current, no change needed
        return false;
      }
      break;
    }
  }

  if (!found) {
    // No "Last updated:" header found, skip this file
    return false;
  }

  const modified = lines.join('\n');

  // Restore original line endings (if the file had CRLF, use CRLF; else \n)
  const hasOriginalCRLF = content.includes('\r\n');
  const final = hasOriginalCRLF ? modified.replace(/\n/g, '\r\n') : modified;

  // Only write if content actually changed
  if (final !== content) {
    writeFileSync(filePath, final, 'utf-8');
    return true;
  }

  return false;
}

/**
 * Main: process argv as file paths.
 */
async function main() {
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
        // Git date unavailable (untracked file, etc.); use today
        newDate = getTodayDate();
      }
    } else {
      newDate = getTodayDate();
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

main().catch((err) => {
  process.stderr.write(`Error: ${err.message}\n`);
  process.exit(1);
});
