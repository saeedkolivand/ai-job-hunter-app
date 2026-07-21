import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, rmSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const scriptPath = join(__dirname, 'ci-review-verdict.mjs');

// review-comment.md is written to cwd, so each test runs in its own temp dir.
const testTmpDir = join(tmpdir(), `ci-review-verdict-test-${Date.now()}`);

const finding = (overrides) => ({
  category: 'correctness',
  file: 'a.ts',
  line: 1,
  summary: 'summary',
  evidence: 'evidence',
  fix: 'fix it',
  confidence: 1,
  introduced_by_diff: true,
  ...overrides,
});

/** Run the script against a fixture findings file written into testTmpDir. */
function run(findings) {
  const findingsPath = join(testTmpDir, 'findings.json');
  writeFileSync(findingsPath, JSON.stringify({ schema: 1, findings }));
  try {
    execFileSync('node', [scriptPath, findingsPath], { cwd: testTmpDir, encoding: 'utf8' });
    return { exitCode: 0, comment: readFileSync(join(testTmpDir, 'review-comment.md'), 'utf8') };
  } catch (err) {
    return {
      exitCode: err.status,
      comment: readFileSync(join(testTmpDir, 'review-comment.md'), 'utf8'),
    };
  }
}

describe('ci-review-verdict sticky comment rendering', () => {
  beforeEach(() => mkdirSync(testTmpDir, { recursive: true }));
  afterEach(() => rmSync(testTmpDir, { recursive: true, force: true }));

  it('exits 0 with an OK headline and verdict line when no findings', () => {
    const { exitCode, comment } = run([]);
    expect(exitCode).toBe(0);
    expect(comment).toContain('## 🤖 AI Review OK');
    expect(comment).toContain('**Verdict: no blocking findings**');
  });

  it('exits 1 and groups a blocking HIGH finding under a severity section, marked 🚫', () => {
    const { exitCode, comment } = run([
      finding({ severity: 'HIGH', file: 'x.ts', line: 42, confidence: 0.9 }),
    ]);
    expect(exitCode).toBe(1);
    expect(comment).toContain('## 🤖 AI Review FAILED');
    expect(comment).toContain('**Verdict: 1 blocking finding(s)**');
    expect(comment).toContain('### 🟠 High');
    expect(comment).toContain('**`x.ts:42`** 🚫 — summary');
    expect(comment).toContain('**Fix:** fix it');
  });

  it('does not mark a HIGH finding below the confidence threshold as blocking', () => {
    const { exitCode, comment } = run([
      finding({ severity: 'HIGH', file: 'x.ts', line: 1, confidence: 0.5 }),
    ]);
    expect(exitCode).toBe(0);
    expect(comment).toContain('## 🤖 AI Review OK');
    expect(comment).not.toContain('🚫');
  });

  it('collapses LOW findings into a <details> block and skips empty severity sections', () => {
    const { comment } = run([finding({ severity: 'LOW', file: 'y.ts', line: 5 })]);
    expect(comment).toContain('<details><summary>⚪ Low (1, advisory)</summary>');
    expect(comment).toContain('- **`y.ts:5`** — summary. _Fix:_ fix it');
    expect(comment).not.toContain('### 🔴 Critical');
    expect(comment).not.toContain('### 🟠 High');
    expect(comment).not.toContain('### 🟡 Medium');
  });
});
