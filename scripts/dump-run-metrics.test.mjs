import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { DatabaseSync } from 'node:sqlite';
import { mkdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const scriptPath = join(__dirname, 'dump-run-metrics.mjs');

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

/** A posting url — the one column in this table that is user data. */
const JOB_A = 'https://boards.example.com/jane-secret-posting-a';
const JOB_B = 'https://boards.example.com/jane-secret-posting-b';

const m = (o) => JSON.stringify(o);

const ROWS = [
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
  // A different `kind` — these tables host every staged run.
  ['a1', JOB_B, 'agent', 'full', 'completed', T0, T0 + 5_000, 'done', m({ calls: 2 })],
];

function run(args) {
  return execFileSync(process.execPath, [scriptPath, ...args], { encoding: 'utf8' });
}

/** Run expecting a non-zero exit; returns `{ status, stderr }`. */
function runFailing(args) {
  try {
    execFileSync(process.execPath, [scriptPath, ...args], { encoding: 'utf8', stdio: 'pipe' });
    throw new Error('expected a non-zero exit');
  } catch (e) {
    return { status: e.status, stderr: String(e.stderr ?? '') };
  }
}

beforeAll(() => {
  mkdirSync(workDir, { recursive: true });
  const db = new DatabaseSync(dbPath);
  db.exec(CREATE_SQL);
  const stmt = db.prepare(
    `INSERT INTO pipeline_runs
       (id, job_url, kind, depth, status, started_at, finished_at, stopped_reason, metrics_json)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
  );
  for (const row of ROWS) stmt.run(...row);
  db.close();
});

afterAll(() => rmSync(workDir, { recursive: true, force: true }));

describe('dump-run-metrics', () => {
  it('documents the default app-data location in --help', () => {
    const out = run(['--help']);
    expect(out).toContain('pipeline_runs.db');
    expect(out).toContain('com.ajh.desktop');
    expect(out).toContain('AJH_DATA_DIR');
  });

  it('aggregates per depth, in pipeline order', () => {
    const { depths } = JSON.parse(run([dbPath, '--json']));
    expect(depths.map((d) => d.depth)).toEqual(['fast', 'max']);

    const fast = depths[0];
    expect(fast.runs).toBe(2);
    expect(fast.postings).toBe(2);
    expect(fast.issuesMean).toBe(4); // (2 + 6) / 2
    expect(fast.criticalsMean).toBe(1); // (0 + 2) / 2
    expect(fast.warningsMean).toBe(3); // ((2-0) + (6-2)) / 2
    expect(fast.msMedian).toBe(20_000); // nearest-rank p50 of [20k, 30k]
    expect(fast.msP90).toBe(30_000);
    expect(fast.statuses).toEqual({ completed: 1, needsReview: 1 });
    expect(fast.stopReasons).toEqual({ done: 2 });
  });

  it('counts a run whose metrics_json was clamped unparseable instead of failing', () => {
    const { depths } = JSON.parse(run([dbPath, '--json']));
    const max = depths.find((d) => d.depth === 'max');
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

  it('filters by run kind, so agent runs never enter a résumé-depth average', () => {
    const resume = JSON.parse(run([dbPath, '--json']));
    expect(resume.depths.flatMap((d) => Object.keys(d.statuses)).length).toBeGreaterThan(0);
    expect(resume.depths.map((d) => d.depth)).not.toContain('full');

    const agent = JSON.parse(run([dbPath, '--json', '--kind', 'agent']));
    expect(agent.depths.map((d) => d.depth)).toEqual(['full']);
    expect(agent.depths[0].runs).toBe(1);
  });

  it('never prints a posting url (ADR-027: counts, codes and durations only)', () => {
    for (const out of [run([dbPath]), run([dbPath, '--json'])]) {
      expect(out).not.toContain(JOB_A);
      expect(out).not.toContain(JOB_B);
      expect(out).not.toContain('boards.example.com');
    }
    // The count derived from those urls is what the table carries instead.
    expect(run([dbPath])).toMatch(/\bjobs\b/);
  });

  it('exits non-zero with a readable message when the database is missing', () => {
    const { status, stderr } = runFailing([join(workDir, 'nope.db')]);
    expect(status).toBe(1);
    expect(stderr).toContain('no database at');
  });

  it('rejects an unknown option rather than silently ignoring it', () => {
    const { status, stderr } = runFailing([dbPath, '--depth', 'max']);
    expect(status).toBe(2);
    expect(stderr).toContain('unknown option: --depth');
  });
});
