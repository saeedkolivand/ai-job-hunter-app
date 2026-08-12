// Dev-only depth A/B dump: what `fast` / `quality` / `max` runs actually cost and
// how clean their output was, read out of the app's own `pipeline_runs.db`.
//
// The offline harness (`apps/desktop/src-tauri/tests/eval.rs`) measures the
// DETERMINISTIC validators on labelled fixtures. It cannot say anything about
// generation, because generation needs a live model — so the depth comparison is
// this: aggregate the `metrics_json` blob every finished run already writes, per
// depth, over the runs a developer produced on their own machine.
//
// CONTENT-FREE by construction (ADR-027): it reads exactly six columns plus
// `metrics_json` (counts, durations, codes) and prints counts, codes and
// milliseconds. It never reads — and cannot print — a résumé, a job ad, an
// evidence span, or a posting URL. `pipeline_run_events.artifact_json` is where a
// max run persists its strategy and evidence detail, and this script does not open
// that table at all.
//
// It opens the database `readOnly`, so no row can change and the main DB file
// stays byte-identical — but "read-only" is not "touches nothing on disk": these
// stores are WAL (ADR-022), and a WAL reader materializes the `-shm`/`-wal`
// sidecars beside the file. See the open site below for why `immutable=1` is the
// wrong trade here.
//
// Source of truth for the shape: apps/desktop/src-tauri/src/pipeline/runs/mod.rs
// (the table) and apps/desktop/src-tauri/src/commands/resume_pipeline/mod.rs
// (the `metrics_json` keys, written at the one terminal-update site).
//
// Usage:  node scripts/dump-run-metrics.mjs --help
//
// Requires Node >= 22.13 for the built-in `node:sqlite` (zero new dependencies —
// the repo carries no JS SQLite driver, and a dev dump script is not worth one).

import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

const DB_FILE = 'pipeline_runs.db';
const IDENTIFIER = 'com.ajh.desktop';

/** Depths in pipeline order, so the table reads fast → max regardless of row order. */
const DEPTH_ORDER = ['fast', 'quality', 'max'];

/**
 * The app data directory the desktop app resolves at runtime — Tauri's
 * `app_data_dir()`, overridable by `AJH_DATA_DIR` (which the app itself exports
 * during setup; see platform/config.rs `resolve_and_export_data_dir`). The
 * per-OS paths are the ones documented in docs/DEVELOPMENT.md.
 */
function defaultDataDir() {
  if (process.env.AJH_DATA_DIR) return process.env.AJH_DATA_DIR;
  const home = homedir();
  if (process.platform === 'win32') {
    return join(process.env.APPDATA || join(home, 'AppData', 'Roaming'), IDENTIFIER);
  }
  if (process.platform === 'darwin') {
    return join(home, 'Library', 'Application Support', IDENTIFIER);
  }
  return join(process.env.XDG_DATA_HOME || join(home, '.local', 'share'), IDENTIFIER);
}

const HELP = `dump-run-metrics — per-depth aggregates over recorded pipeline runs (dev-only)

  node scripts/dump-run-metrics.mjs [<path-to-${DB_FILE}>] [options]

Options
  --db <path>     The database to read. Defaults to the app data dir's ${DB_FILE}:
                    Windows  %APPDATA%\\${IDENTIFIER}\\${DB_FILE}
                    macOS    ~/Library/Application Support/${IDENTIFIER}/${DB_FILE}
                    Linux    ~/.local/share/${IDENTIFIER}/${DB_FILE}
                  \`AJH_DATA_DIR\` overrides the directory, exactly as it does for
                  the app itself.
  --kind <kind>   Run kind to include (default: resume). \`kind\` is the store's
                  discriminator — these tables also host agent runs.
  --json          Emit the aggregates as JSON instead of a table.
  --help, -h      This text.

Output is counts, codes and durations only — never document text (ADR-027); the
template paths above are printed unexpanded for the same reason.
Requires Node >= 22.13 (built-in node:sqlite, unflagged).`;

/**
 * Parse argv into options.
 *
 * A value that begins with `-` is REFUSED rather than consumed: `--kind --json`
 * is a typo, and silently taking `--json` as the kind name would have queried
 * for runs of kind "--json" (zero rows) and dropped the flag — a wrong answer
 * reported confidently.
 */
function parseArgs(argv) {
  const opts = { db: null, kind: 'resume', json: false, help: false };
  const value = (flag, raw) => {
    if (raw === undefined) throw new Error(`${flag} needs a value`);
    if (raw.startsWith('-')) throw new Error(`${flag} needs a value, got the flag ${raw}`);
    return raw;
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') opts.help = true;
    else if (arg === '--json') opts.json = true;
    else if (arg === '--db') opts.db = value('--db', argv[(i += 1)]);
    else if (arg === '--kind') opts.kind = value('--kind', argv[(i += 1)]);
    else if (arg.startsWith('-')) throw new Error(`unknown option: ${arg}`);
    else if (opts.db === null) opts.db = arg;
    else throw new Error(`unexpected extra argument: ${arg}`);
  }
  return opts;
}

/**
 * `p`-th percentile of `values` by nearest-rank, or null for an empty set.
 *
 * Nearest-rank (not interpolated) on purpose: these are run durations measured a
 * handful of times on one machine, and an interpolated p90 over six samples
 * invents a millisecond value no run ever took.
 */
function percentile(values, p) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(Math.max(rank, 1), sorted.length) - 1];
}

function mean(values) {
  if (values.length === 0) return null;
  return values.reduce((a, b) => a + b, 0) / values.length;
}

/** `n` → a fixed-width count string; null → an em dash, never a misleading `0`. */
function num(value, digits = 1) {
  if (value === null || value === undefined) return '—';
  return Number.isInteger(value) ? String(value) : value.toFixed(digits);
}

function tally(map, key) {
  map.set(key, (map.get(key) ?? 0) + 1);
}

function formatTally(map) {
  if (map.size === 0) return '—';
  return [...map.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .map(([key, count]) => `${key}=${count}`)
    .join(' ');
}

/**
 * Fold one depth's rows into the aggregate row the table prints.
 *
 * Every field is derived defensively: `metrics_json` is a free-form, CLAMPED
 * column (`METRICS_CAP_BYTES`), an over-cap value is deliberately left
 * UNPARSEABLE by the store's truncation marker, and rows written by an older
 * build simply lack the newer keys. A dump that threw on any of those would be
 * unusable against exactly the history it exists to read, so unreadable metrics
 * are COUNTED (`unparsedMetrics`) rather than silently treated as zeros.
 */
function aggregate(depth, rows) {
  const statuses = new Map();
  const stops = new Map();
  const durations = [];
  const issues = [];
  const criticals = [];
  const repairs = [];
  const calls = [];
  const cached = [];
  const warnings = [];
  let unparsedMetrics = 0;
  let reverted = 0;
  // How many rows actually REPORTED a `reverted` flag. Without this, a row whose
  // metrics parse but carry none of the expected keys made the column print a
  // confident `0` ("no run reverted") while every neighbouring cell printed `—`
  // ("nobody measured"). Zero and unmeasured are different answers.
  let revertedKnown = 0;
  const postings = new Set();

  for (const row of rows) {
    tally(statuses, row.status);
    tally(stops, row.stopped_reason ?? '—');
    // A count of DISTINCT postings, never the urls themselves: "6 runs over 2
    // postings" is what makes an average meaningful, and the url is the one
    // column here that is user data.
    postings.add(row.job_url ?? '');

    // ONE parse per row: the warnings pass used to re-parse every row a second
    // time, which is two chances for the two passes to disagree about what a
    // row said.
    let metrics = null;
    try {
      metrics = JSON.parse(row.metrics_json ?? '{}');
    } catch {
      unparsedMetrics += 1;
    }
    if (metrics && typeof metrics === 'object') {
      if (typeof metrics.ms === 'number') durations.push(metrics.ms);
      if (typeof metrics.issueCount === 'number') issues.push(metrics.issueCount);
      if (typeof metrics.criticalCount === 'number') criticals.push(metrics.criticalCount);
      if (typeof metrics.repairRounds === 'number') repairs.push(metrics.repairRounds);
      if (typeof metrics.calls === 'number') calls.push(metrics.calls);
      if (typeof metrics.cached === 'number') cached.push(metrics.cached);
      if (typeof metrics.reverted === 'boolean') {
        revertedKnown += 1;
        if (metrics.reverted) reverted += 1;
      }
      // Warnings = issues − criticals, from the SAME run: subtracting two
      // independently-averaged columns would report a warning count for runs
      // that never reported one.
      if (typeof metrics.issueCount === 'number' && typeof metrics.criticalCount === 'number') {
        warnings.push(metrics.issueCount - metrics.criticalCount);
      }
    }
    // `ms` is written only at the terminal update, so a crashed/still-running
    // row has none. The columns still bound it — but only when it FINISHED;
    // an open run has no duration, not a duration of "now minus then".
    if (
      (!metrics || typeof metrics.ms !== 'number') &&
      typeof row.finished_at === 'number' &&
      typeof row.started_at === 'number'
    ) {
      durations.push(row.finished_at - row.started_at);
    }
  }

  return {
    depth,
    runs: rows.length,
    postings: postings.size,
    statuses: Object.fromEntries(statuses),
    stopReasons: Object.fromEntries(stops),
    msMedian: percentile(durations, 50),
    msP90: percentile(durations, 90),
    issuesMean: mean(issues),
    criticalsMean: mean(criticals),
    warningsMean: mean(warnings),
    repairRoundsMean: mean(repairs),
    // `null` when no row reported the flag at all — see `revertedKnown`.
    revertedRuns: revertedKnown > 0 ? reverted : null,
    callsMean: mean(calls),
    cachedMean: mean(cached),
    unparsedMetrics,
  };
}

function printTable(aggregates, kind) {
  const total = aggregates.reduce((sum, a) => sum + a.runs, 0);
  console.log(
    `\n=== pipeline run metrics — kind=${kind}, ${total} run${total === 1 ? '' : 's'} ===\n`
  );
  const head = [
    'depth'.padEnd(9),
    'runs'.padStart(5),
    'jobs'.padStart(5),
    'ms p50'.padStart(9),
    'ms p90'.padStart(9),
    'issues'.padStart(7),
    'crit'.padStart(6),
    'warn'.padStart(6),
    'repairs'.padStart(8),
    'revert'.padStart(7),
    'calls'.padStart(6),
    'cached'.padStart(7),
  ].join(' ');
  console.log(head);
  for (const a of aggregates) {
    console.log(
      [
        a.depth.padEnd(9),
        String(a.runs).padStart(5),
        String(a.postings).padStart(5),
        num(a.msMedian).padStart(9),
        num(a.msP90).padStart(9),
        num(a.issuesMean).padStart(7),
        num(a.criticalsMean).padStart(6),
        num(a.warningsMean).padStart(6),
        num(a.repairRoundsMean).padStart(8),
        num(a.revertedRuns).padStart(7),
        num(a.callsMean).padStart(6),
        num(a.cachedMean).padStart(7),
      ].join(' ')
    );
  }
  console.log('\nmeans per run; ms p50/p90 are nearest-rank over the runs that finished\n');
  console.log(`${'depth'.padEnd(9)} ${'status'.padEnd(44)} stopped reason`);
  for (const a of aggregates) {
    console.log(
      `${a.depth.padEnd(9)} ${formatTally(new Map(Object.entries(a.statuses))).padEnd(44)} ` +
        formatTally(new Map(Object.entries(a.stopReasons)))
    );
  }
  const unparsed = aggregates.reduce((sum, a) => sum + a.unparsedMetrics, 0);
  if (unparsed > 0) {
    console.log(
      `\n${unparsed} run(s) carry unparseable metrics_json (clamped past ` +
        `METRICS_CAP_BYTES) and contributed to counts only.`
    );
  }
  console.log();
}

async function main() {
  let opts;
  try {
    opts = parseArgs(process.argv.slice(2));
  } catch (e) {
    console.error(`${e.message}\n\n${HELP}`);
    process.exit(2);
  }
  if (opts.help) {
    console.log(HELP);
    return;
  }

  // An EXPLICIT path is echoed back (the user typed it); a DEFAULTED one is
  // named by its template rather than expanded, because the resolved form
  // contains the OS account name and these messages get pasted into issues.
  const defaulted = opts.db === null;
  const dbPath = opts.db ?? join(defaultDataDir(), DB_FILE);
  const shownPath = defaulted ? `<app data dir>/${DB_FILE}` : dbPath;
  if (!existsSync(dbPath)) {
    console.error(
      `no database at ${shownPath}\n\nRun the résumé pipeline at least once, or pass the path ` +
        `explicitly. \`node scripts/dump-run-metrics.mjs --help\` lists the defaults.`
    );
    process.exit(1);
  }

  let DatabaseSync;
  try {
    ({ DatabaseSync } = await import('node:sqlite'));
  } catch {
    console.error(
      `node:sqlite is unavailable on ${process.version} — this dev script needs Node >= 22.13.`
    );
    process.exit(1);
  }

  // `readOnly` because this may be the LIVE app database: no statement here can
  // change a row, and the main DB file's bytes are untouched.
  //
  // It does NOT mean "leaves the directory alone". Every store opens WAL
  // (`db::open`, ADR-022), and a WAL READER still maps the shared-memory index
  // and the log — so `-shm` (and `-wal`, if none exists yet) appear beside the
  // database and the directory must be writable. That is normal SQLite
  // behaviour, pinned by `a WAL dump leaves the main database byte-identical` in
  // the sibling test; `immutable=1` would avoid the sidecars but is a promise
  // this cannot keep, because the app may be writing while the dump reads.
  const db = new DatabaseSync(dbPath, { readOnly: true });
  let rows;
  try {
    rows = db
      .prepare(
        `SELECT depth, status, started_at, finished_at, stopped_reason, job_url, metrics_json
           FROM pipeline_runs
          WHERE kind = ?
          ORDER BY started_at`
      )
      .all(opts.kind);
  } catch (e) {
    // The only schema-drift alarm in the script: a renamed table or column
    // reaches here, and reporting "no runs" for it would read as "you have not
    // run the pipeline yet" — the one wrong answer that stops the reader
    // looking.
    console.error(`cannot read pipeline_runs from ${shownPath}: ${e.message}`);
    process.exit(1);
  } finally {
    db.close();
  }

  if (rows.length === 0) {
    console.error(`no runs with kind=${opts.kind} in ${shownPath}`);
    process.exit(1);
  }

  const byDepth = new Map();
  for (const row of rows) {
    const depth = row.depth || '(unset)';
    if (!byDepth.has(depth)) byDepth.set(depth, []);
    byDepth.get(depth).push(row);
  }
  // Known depths first, in pipeline order; anything else (an older build, a
  // renamed depth) after them rather than dropped.
  const depths = [
    ...DEPTH_ORDER.filter((d) => byDepth.has(d)),
    ...[...byDepth.keys()].filter((d) => !DEPTH_ORDER.includes(d)).sort(),
  ];
  const aggregates = depths.map((depth) => aggregate(depth, byDepth.get(depth)));

  if (opts.json) {
    console.log(JSON.stringify({ kind: opts.kind, depths: aggregates }, null, 2));
    return;
  }
  printTable(aggregates, opts.kind);
}

await main();
