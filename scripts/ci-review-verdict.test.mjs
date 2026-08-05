import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, rmSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { inlineFindings, ledgerKey } from '../.claude/hooks/review-lib.mjs';

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

// The "📌 Inline findings on the diff" step decides what to pin from these two
// helpers, so the rules live here rather than in unrunnable workflow YAML.
describe('inline finding selection', () => {
  it('pins CRITICAL, HIGH and MEDIUM but leaves LOW to the sticky comment', () => {
    const picked = inlineFindings([
      finding({ severity: 'CRITICAL' }),
      finding({ severity: 'HIGH' }),
      finding({ severity: 'MEDIUM' }),
      finding({ severity: 'LOW' }),
    ]);
    expect(picked.map((f) => f.severity)).toEqual(['CRITICAL', 'HIGH', 'MEDIUM']);
  });

  it('skips a pre-existing finding — inline comments are for what the diff introduced', () => {
    expect(inlineFindings([finding({ severity: 'HIGH', introduced_by_diff: false })])).toEqual([]);
  });

  it('skips a finding that cannot be anchored, rather than letting the API reject it', () => {
    // GitHub requires a path AND a line in the diff; a finding missing either
    // must stay sticky-only instead of 422-ing the post.
    expect(inlineFindings([finding({ severity: 'HIGH', line: 0 })])).toEqual([]);
    expect(inlineFindings([finding({ severity: 'HIGH', line: undefined })])).toEqual([]);
    expect(inlineFindings([finding({ severity: 'HIGH', file: '' })])).toEqual([]);
  });

  it('keeps a finding whose confidence is below the BLOCKING bar', () => {
    // Inline is not gated on confidence — that bar decides the merge verdict.
    // A MEDIUM at 0.5 is still worth showing next to the code it concerns.
    expect(inlineFindings([finding({ severity: 'MEDIUM', confidence: 0.5 })])).toHaveLength(1);
  });

  it('gives the same finding a stable key across runs even when its line moves', () => {
    // This is what stops every push re-posting the same comment: an edit above
    // a finding shifts its line, and a line-keyed identity would look new.
    const a = finding({ severity: 'HIGH', file: 'x.ts', line: 42 });
    const b = finding({ severity: 'HIGH', file: 'x.ts', line: 108 });
    expect(ledgerKey(a)).toBe(ledgerKey(b));
  });

  it('gives a different key to a different finding in the same file', () => {
    const a = finding({ severity: 'HIGH', file: 'x.ts', summary: 'unbounded read' });
    const b = finding({ severity: 'HIGH', file: 'x.ts', summary: 'missing null check' });
    expect(ledgerKey(a)).not.toBe(ledgerKey(b));
  });
});
