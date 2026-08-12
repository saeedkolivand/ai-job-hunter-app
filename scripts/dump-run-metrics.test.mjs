import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const scriptPath = join(__dirname, 'dump-run-metrics.mjs');

// `node:sqlite` needs Node >= 22.13, but the repo's `engines` still permits
// 20.11 — so a static import would take the WHOLE scripts project down at load
// on a permitted Node, turning "this one dev script is unavailable" into "the JS
// suite is broken". Probe instead: skip locally with a reason, and FAIL LOUDLY in
// CI, where the runner's Node is pinned and a silent skip would quietly retire
// these tests.
let DatabaseSync = null;
try {
  ({ DatabaseSync } = await import('node:sqlite'));
} catch (e) {
  if (process.env.CI) {
    throw new Error(
      `node:sqlite is unavailable on ${process.version}; CI must run a Node that has it ` +
        `(>= 22.13) or dump-run-metrics is untested`,
      { cause: e }
    );
  }
  console.warn(
    `[dump-run-metrics.test] skipped: node:sqlite unavailable on ${process.version} (needs >= 22.13)`
  );
}
const describeSqlite = DatabaseSync ? describe : describe.skip;

// Black-box test of the CLI (execFileSync), mirroring check-tech-radar.test.mjs /
// ci-review-verdict.test.mjs. The fixture DB is created with the schema VERBATIM
// from `CREATE_PIPELINE_RUNS_SQL` in apps/desktop/src-tauri/src/pipeline/runs/mod.rs
// — a hand-simplified schema would let the script pass here and fail on a real
// database.
const CREATE_SQL = `CREATE TABLE IF NOT EXISTS pipeline_runs (
        id             TEXT PRIMARY KEY NOT NULL,
        job_url        TEXT NOT NULL,
        kind           TEXT NOT NULL,
        depth          TEXT NOT NULL,
        status         TEXT NOT NULL,
        started_at     INTEGER NOT NULL,
        finished_at    INTEGER,
        stopped_reason TEXT,
        metrics_json   TEXT NOT NULL DEFAULT '{}'
     );
     CREATE INDEX IF NOT EXISTS idx_pipeline_runs_job
         ON pipeline_runs(job_url, started_at DESC);
     CREATE TABLE IF NOT EXISTS pipeline_run_events (
        run_id        TEXT NOT NULL,
        seq           INTEGER NOT NULL,
        ts            INTEGER NOT NULL,
        stage         TEXT NOT NULL,
        phase         TEXT NOT NULL CHECK (phase IN ('start', 'finish', 'error')),
        artifact_json TEXT NOT NULL,
        PRIMARY KEY (run_id, seq)
     );`;

const workDir = join(tmpdir(), `dump-run-metrics-test-${Date.now()}`);
const dbPath = join(workDir, 'pipeline_runs.db');
const T0 = 1_770_000_000_000;

/** Posting urls — the one column in this table that is user data. */
const JOB_A = 'https://boards.example.com/jane-secret-posting-a';
const JOB_B = 'https://boards.example.com/jane-secret-posting-b';
/** `sourceResumeId` rides inside metrics_json as provenance; it must not print. */
const RESUME_ID = 'doc-secret-provenance-1';

const m = (o) => JSON.stringify(o);

// INSERTION ORDER IS PART OF THE FIXTURE: rows go in as legacy → unset → max →
// quality → fast, and every row shares `started_at`, so the query returns them
// in that order. The table must still print fast → quality → max → the rest,
// which is only true if DEPTH_ORDER (and the sorted tail) actually do work.
const ROWS = [
  // A depth from an older build, and a row with no depth at all.
  [
    'x1',
    JOB_A,
    'resume',
    'legacy-deep',
    'completed',
    T0,
    T0 + 9_000,
    'done',
    m({
      calls: 1,
      cached: 0,
      repairRounds: 0,
      reverted: false,
      ms: 9_000,
      issueCount: 0,
      criticalCount: 0,
    }),
  ],
  [
    'x2',
    JOB_A,
    'resume',
    '',
    'completed',
    T0,
    T0 + 8_000,
    'done',
    m({
      calls: 1,
      cached: 0,
      repairRounds: 0,
      reverted: false,
      ms: 8_000,
      issueCount: 1,
      criticalCount: 0,
    }),
  ],
  // max: one finished, one still running (no finished_at, no metrics), one whose
  // metrics_json was clamped past METRICS_CAP_BYTES and is therefore unparseable
  // BY DESIGN — but which still has both timestamps.
  [
    'r3',
    JOB_A,
    'resume',
    'max',
    'completed',
    T0,
    T0 + 300_000,
    'max_repairs',
    m({
      calls: 18,
      cached: 4,
      repairRounds: 3,
      reverted: true,
      ms: 300_000,
      issueCount: 4,
      criticalCount: 1,
    }),
  ],
  ['r4', JOB_A, 'resume', 'max', 'running', T0, null, null, '{}'],
  [
    'r5',
    JOB_A,
    'resume',
    'max',
    'completed',
    T0,
    T0 + 100_000,
    'done',
    '{"calls":12,"repairRo…[truncated]',
  ],
  // quality: the PRODUCTION shape for a run whose report was never built —
  // `issueCount` is an Option in Rust and serializes to null.
  [
    'q1',
    JOB_B,
    'resume',
    'quality',
    'failed',
    T0,
    T0 + 61_000,
    'run_timeout',
    m({
      calls: 6,
      cached: 1,
      repairRounds: 2,
      reverted: false,
      ms: 61_000,
      issueCount: null,
      criticalCount: 0,
      sourceResumeId: RESUME_ID,
    }),
  ],
  // fast: two finished runs over two postings.
  [
    'r1',
    JOB_A,
    'resume',
    'fast',
    'completed',
    T0,
    T0 + 20_000,
    'done',
    m({
      calls: 3,
      cached: 0,
      repairRounds: 0,
      reverted: false,
      ms: 20_000,
      issueCount: 2,
      criticalCount: 0,
      sourceResumeId: RESUME_ID,
    }),
  ],
  [
    'r2',
    JOB_B,
    'resume',
    'fast',
    'needsReview',
    T0,
    T0 + 30_000,
    'done',
    m({
      calls: 3,
      cached: 1,
      repairRounds: 0,
      reverted: false,
      ms: 30_000,
      issueCount: 6,
      criticalCount: 2,
    }),
  ],
  // A different `kind` — these tables host every staged run.
  ['a1', JOB_B, 'agent', 'full', 'completed', T0, T0 + 5_000, 'done', m({ calls: 2 })],
];

function run(args) {
  return execFileSync(process.execPath, [scriptPath, ...args], { encoding: 'utf8' });
}

/** Run expecting a non-zero exit; returns `{ status, stderr }`. */
function runFailing(args) {
  let stdout;
  try {
    stdout = execFileSync(process.execPath, [scriptPath, ...args], {
      encoding: 'utf8',
      stdio: 'pipe',
    });
  } catch (e) {
    return { status: e.status, stderr: String(e.stderr ?? '') };
  }
  // OUTSIDE the catch: a `throw` inside it would be caught by its own handler and
  // reported as `status: undefined`, i.e. the assertion would pass by accident.
  throw new Error(`expected a non-zero exit from ${args.join(' ')}; got 0 with: ${stdout}`);
}

function md5(path) {
  return createHash('md5').update(readFileSync(path)).digest('hex');
}

function seed(path, rows) {
  const db = new DatabaseSync(path);
  // The product opens every store in WAL (db.rs `open`, ADR-022), so the fixture
  // must be WAL too — a rollback-journal fixture would not exercise what a dump
  // of a real database does to the directory.
  db.exec('PRAGMA journal_mode = WAL');
  db.exec(CREATE_SQL);
  const stmt = db.prepare(
    `INSERT INTO pipeline_runs
       (id, job_url, kind, depth, status, started_at, finished_at, stopped_reason, metrics_json)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
  );
  for (const row of rows) stmt.run(...row);
  db.close();
}

beforeAll(() => {
  mkdirSync(workDir, { recursive: true });
  if (DatabaseSync) seed(dbPath, ROWS);
});

afterAll(() => rmSync(workDir, { recursive: true, force: true }));

describeSqlite('dump-run-metrics', () => {
  it('documents the default app-data location in --help', () => {
    const out = run(['--help']);
    expect(out).toContain('pipeline_runs.db');
    expect(out).toContain('com.ajh.desktop');
    expect(out).toContain('AJH_DATA_DIR');
  });

  it('aggregates per depth', () => {
    const { depths } = JSON.parse(run([dbPath, '--json']));

    const fast = depths.find((d) => d.depth === 'fast');
    expect(fast.runs).toBe(2);
    expect(fast.postings).toBe(2);
    expect(fast.issuesMean).toBe(4); // (2 + 6) / 2
    expect(fast.criticalsMean).toBe(1); // (0 + 2) / 2
    expect(fast.warningsMean).toBe(3); // ((2-0) + (6-2)) / 2
    expect(fast.msMedian).toBe(20_000); // nearest-rank p50 of [20k, 30k]
    expect(fast.msP90).toBe(30_000);
    expect(fast.revertedRuns).toBe(0); // both rows REPORTED false
    expect(fast.statuses).toEqual({ completed: 1, needsReview: 1 });
    expect(fast.stopReasons).toEqual({ done: 2 });
  });

  it('orders depths fast → quality → max, then anything unrecognized', () => {
    const { depths } = JSON.parse(run([dbPath, '--json']));
    // Insertion (and therefore query) order is legacy-deep, (unset), max,
    // quality, fast — so this equality only holds because the script reorders.
    expect(depths.map((d) => d.depth)).toEqual([
      'fast',
      'quality',
      'max',
      '(unset)',
      'legacy-deep',
    ]);
    // A row with an empty `depth` is labelled, not dropped.
    expect(depths.find((d) => d.depth === '(unset)').runs).toBe(1);
  });

  it('reports a null issueCount as unmeasured rather than zero', () => {
    // The Rust side writes `Option<usize>` → `null` whenever the run produced no
    // report. Averaging that as 0 would claim a clean document.
    const quality = JSON.parse(run([dbPath, '--json'])).depths.find((d) => d.depth === 'quality');
    expect(quality.runs).toBe(1);
    expect(quality.issuesMean).toBeNull();
    expect(quality.warningsMean).toBeNull(); // the pair is incomplete, so no subtraction
    expect(quality.criticalsMean).toBe(0); // …but this half WAS reported
    expect(quality.msMedian).toBe(61_000);
    expect(quality.stopReasons).toEqual({ run_timeout: 1 });
  });

  it('counts a run whose metrics_json was clamped unparseable instead of failing', () => {
    const max = JSON.parse(run([dbPath, '--json'])).depths.find((d) => d.depth === 'max');
    expect(max.runs).toBe(3);
    expect(max.unparsedMetrics).toBe(1);
    // The unreadable row still has timestamps, so it contributes a duration; the
    // RUNNING row has no `finished_at` and must contribute none — a p50 of
    // [100k, 300k] rather than of [0, 100k, 300k].
    expect(max.msMedian).toBe(100_000);
    expect(max.msP90).toBe(300_000);
    // …but its unreadable counts are absent, not zero: only r3 reported issues.
    expect(max.issuesMean).toBe(4);
    expect(max.revertedRuns).toBe(1);
    expect(max.statuses).toEqual({ completed: 2, running: 1 });
  });

  it('prints an em dash, not 0, for a flag no row reported', () => {
    // A row whose metrics parse but carry none of the expected keys (an older
    // build, a renamed field). `revert 0` would read as "nothing reverted"; the
    // truth is that nobody measured.
    const legacyDb = join(workDir, 'legacy.db');
    seed(legacyDb, [
      [
        'l1',
        JOB_A,
        'resume',
        'fast',
        'completed',
        T0,
        T0 + 1_000,
        'done',
        '{"repair_rounds":3,"was_reverted":true}',
      ],
    ]);
    const { depths } = JSON.parse(run([legacyDb, '--json']));
    expect(depths[0].revertedRuns).toBeNull();
    expect(run([legacyDb])).toMatch(/^fast\s+1\s+1\s+1000\s+1000\s+—\s+—\s+—\s+—\s+—\s/m);
  });

  it('filters by run kind, so agent runs never enter a résumé-depth average', () => {
    const resume = JSON.parse(run([dbPath, '--json']));
    expect(resume.depths.map((d) => d.depth)).not.toContain('full');

    const agent = JSON.parse(run([dbPath, '--json', '--kind', 'agent']));
    expect(agent.depths.map((d) => d.depth)).toEqual(['full']);
    expect(agent.depths[0].runs).toBe(1);
  });

  it('accepts the path as --db as well as positionally', () => {
    expect(run(['--db', dbPath, '--json'])).toBe(run([dbPath, '--json']));
  });

  it('never prints a posting url or a resume id (ADR-027)', () => {
    for (const out of [run([dbPath]), run([dbPath, '--json'])]) {
      expect(out).not.toContain(JOB_A);
      expect(out).not.toContain(JOB_B);
      expect(out).not.toContain('boards.example.com');
      expect(out).not.toContain(RESUME_ID);
    }
    // The counts derived from those urls are what the table carries instead:
    // the `fast` row's 2 runs over 2 distinct postings, with its durations.
    expect(run([dbPath])).toMatch(/^fast\s+2\s+2\s+20000\s+30000\s/m);
  });

  it('leaves the database content untouched, WAL sidecars aside', () => {
    const walDb = join(workDir, 'wal.db');
    seed(walDb, [
      ['w1', JOB_A, 'resume', 'fast', 'completed', T0, T0 + 1_000, 'done', m({ ms: 1_000 })],
    ]);
    const before = md5(walDb);

    run([walDb]);

    // (a) the database's CONTENT is byte-identical — the actual guarantee.
    expect(md5(walDb)).toBe(before);
    // (b) and the sidecars a WAL READER materializes ARE there. Asserted rather
    // than wished away: the script's comment used to claim it left none, which
    // is impossible for a WAL reader, and a dump therefore needs a WRITABLE
    // directory. Pinned so nobody "fixes" the comment back.
    expect(existsSync(`${walDb}-shm`)).toBe(true);
    expect(existsSync(`${walDb}-wal`)).toBe(true);
  });

  it('exits non-zero with a readable message when the database is missing', () => {
    const { status, stderr } = runFailing([join(workDir, 'nope.db')]);
    expect(status).toBe(1);
    expect(stderr).toContain('no database at');
  });

  it('reports schema drift instead of an empty result', () => {
    // The table renamed or gone is NOT "you have no runs yet" — that message
    // would stop the reader looking. This is the script's only drift alarm.
    const emptyDb = join(workDir, 'no-table.db');
    const db = new DatabaseSync(emptyDb);
    db.exec('PRAGMA journal_mode = WAL');
    db.exec('CREATE TABLE unrelated (id TEXT PRIMARY KEY)');
    db.close();

    const { status, stderr } = runFailing([emptyDb]);
    expect(status).toBe(1);
    expect(stderr).toMatch(/cannot read pipeline_runs/);
  });

  it('exits non-zero when the kind matches no run', () => {
    const { status, stderr } = runFailing([dbPath, '--kind', 'nosuch']);
    expect(status).toBe(1);
    expect(stderr).toContain('no runs with kind=nosuch');
  });

  it('rejects an unknown option rather than silently ignoring it', () => {
    const { status, stderr } = runFailing([dbPath, '--depth', 'max']);
    expect(status).toBe(2);
    expect(stderr).toContain('unknown option: --depth');
  });

  it('rejects a flag where a value belongs, and a missing value', () => {
    // `--kind --json` would otherwise query for runs of kind "--json" (zero
    // rows) AND drop the flag — a confidently wrong answer.
    expect(runFailing([dbPath, '--kind', '--json']).stderr).toContain(
      '--kind needs a value, got the flag --json'
    );
    expect(runFailing([dbPath, '--db']).stderr).toContain('--db needs a value');
  });
});
