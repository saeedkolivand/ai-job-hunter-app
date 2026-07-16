import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, rmSync, readFileSync, writeFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const scriptPath = join(__dirname, 'bump-last-updated.mjs');

// Fixed test date (independent of real clock) — tests set BUMP_DATE env var to this
// so assertions never break from calendar rolls. Chosen arbitrarily in the future.
const TEST_DATE = '2031-01-05';

// Create a unique temp dir for this test run
const testTmpDir = join(tmpdir(), `bump-last-updated-test-${Date.now()}`);

// Helper: run script with BUMP_DATE env var set to TEST_DATE
function runScript(files, options = {}) {
  return execFileSync('node', [scriptPath, ...files], {
    env: { ...process.env, BUMP_DATE: TEST_DATE },
    ...options,
  });
}

describe('bump-last-updated script', () => {
  beforeEach(() => {
    mkdirSync(testTmpDir, { recursive: true });
  });

  afterEach(() => {
    // Clean up temp files
    rmSync(testTmpDir, { recursive: true, force: true });
  });

  describe('(1) date-changed: stale date → today, annotation preserved', () => {
    it('bumps stale date to TEST_DATE preserving annotation', () => {
      const testFile = join(testTmpDir, 'doc-with-annotation.md');
      writeFileSync(
        testFile,
        `# Test Doc

Last updated: 2026-06-01 (v0.116.0)

Some content.
`
      );

      runScript([testFile], { stdio: 'inherit' });

      const content = readFileSync(testFile, 'utf-8');
      expect(content).toContain(`Last updated: ${TEST_DATE} (v0.116.0)`);
      // Verify the annotation is preserved byte-for-byte
      expect(content).toMatch(/Last updated: \d{4}-\d{2}-\d{2} \(v0\.116\.0\)/);
    });

    it('bumps stale date without annotation', () => {
      const testFile = join(testTmpDir, 'doc-no-annotation.md');
      writeFileSync(
        testFile,
        `# Test Doc

Last updated: 2026-05-15

Some content.
`
      );

      runScript([testFile]);

      const content = readFileSync(testFile, 'utf-8');
      expect(content).toContain(`Last updated: ${TEST_DATE}`);
      expect(content).not.toContain('2026-05-15');
    });

    it('preserves trailing text with task reference', () => {
      const testFile = join(testTmpDir, 'doc-with-task.md');
      writeFileSync(
        testFile,
        `# Test Doc

Last updated: 2026-06-01 (task #16: architecture review)

Content here.
`
      );

      runScript([testFile]);

      const content = readFileSync(testFile, 'utf-8');
      expect(content).toContain(`Last updated: ${TEST_DATE} (task #16: architecture review)`);
    });
  });

  describe('(2) idempotence: already-current date → no write', () => {
    it('does not modify file if date is already current', () => {
      const testFile = join(testTmpDir, 'already-current.md');
      const originalContent = `# Test Doc

Last updated: ${TEST_DATE}

Content.
`;
      writeFileSync(testFile, originalContent);

      // Record original mtime
      const mtimeBefore = statSync(testFile).mtimeMs;

      // Wait a tiny bit to ensure mtime would differ if file was written
      runScript([testFile]);

      // Verify content is unchanged
      expect(readFileSync(testFile, 'utf-8')).toBe(originalContent);

      // mtime should not have changed (file was not written)
      const mtimeAfter = statSync(testFile).mtimeMs;
      expect(mtimeAfter).toBe(mtimeBefore);
    });

    it('does not log output when date is already current', () => {
      const testFile = join(testTmpDir, 'current-date.md');
      writeFileSync(
        testFile,
        `# Test

Last updated: ${TEST_DATE}

Content.
`
      );

      try {
        runScript([testFile], {
          encoding: 'utf-8',
          stdio: ['pipe', 'pipe', 'pipe'],
        });
      } catch (err) {
        // Script exits with code 0 normally, so if stderr contains output, it's logged
        if (err.stderr) {
          expect(err.stderr).not.toContain('Last updated bumped');
        }
      }
      // If no error, no stderr was written — that's the expected behavior
    });
  });

  describe('(3) file without header → untouched', () => {
    it('leaves file unchanged if no "Last updated" header exists', () => {
      const testFile = join(testTmpDir, 'no-header.md');
      const originalContent = `# Test Doc

Some content without a last updated header.

## Section

More content.
`;
      writeFileSync(testFile, originalContent);

      runScript([testFile]);

      expect(readFileSync(testFile, 'utf-8')).toBe(originalContent);
    });

    it('ignores "Last updated" in content (not header)', () => {
      const testFile = join(testTmpDir, 'content-mention.md');
      const originalContent = `# Test Doc

Some text mentioning that I wrote this on Last updated: 2026-06-01 but that's in content.

## Real content
`;
      writeFileSync(testFile, originalContent);

      runScript([testFile]);

      expect(readFileSync(testFile, 'utf-8')).toBe(originalContent);
    });
  });

  describe('(4) CRLF file → date bumped and line endings preserved', () => {
    it('preserves CRLF line endings while bumping date', () => {
      const testFile = join(testTmpDir, 'crlf-file.md');
      const contentLF = `# Test Doc\n\nLast updated: 2026-06-01\n\nContent.\n`;
      const contentCRLF = contentLF.replace(/\n/g, '\r\n');
      writeFileSync(testFile, contentCRLF);

      runScript([testFile]);

      const result = readFileSync(testFile, 'utf-8');
      // Verify CRLF is preserved (file should have \r\n, not just \n)
      expect(result).toContain('\r\n');
      expect(result).toContain(`Last updated: ${TEST_DATE}`);
      // Verify the structure (should have CRLF throughout)
      const lines = result.split('\r\n');
      expect(lines.length).toBeGreaterThan(1);
    });

    it('preserves LF line endings while bumping date', () => {
      const testFile = join(testTmpDir, 'lf-file.md');
      const contentLF = `# Test Doc\n\nLast updated: 2026-06-01\n\nContent.\n`;
      writeFileSync(testFile, contentLF);

      runScript([testFile]);

      const result = readFileSync(testFile, 'utf-8');
      // Should not have CRLF (only LF)
      expect(result).not.toContain('\r\n');
      expect(result).toContain(`Last updated: ${TEST_DATE}`);
    });
  });

  describe('(5) non-md path in argv → ignored', () => {
    it('skips non-markdown files', () => {
      const testFile = join(testTmpDir, 'doc.txt');
      const originalContent = `Last updated: 2026-06-01`;
      writeFileSync(testFile, originalContent);

      runScript([testFile]);

      expect(readFileSync(testFile, 'utf-8')).toBe(originalContent);
    });

    it('processes mixed files, ignoring non-md', () => {
      const mdFile = join(testTmpDir, 'doc.md');
      const txtFile = join(testTmpDir, 'note.txt');

      writeFileSync(mdFile, `# Markdown\n\nLast updated: 2026-06-01\n`);
      writeFileSync(txtFile, `Last updated: 2026-06-01`);

      runScript([mdFile, txtFile]);

      // MD file should be bumped
      expect(readFileSync(mdFile, 'utf-8')).toContain(`Last updated: ${TEST_DATE}`);
      // TXT file should be untouched
      expect(readFileSync(txtFile, 'utf-8')).toBe(`Last updated: 2026-06-01`);
    });

    it('exits silently when given no files', () => {
      const result = runScript([], {
        encoding: 'utf-8',
        stdio: ['pipe', 'pipe', 'pipe'],
      });

      // Should exit with code 0 and produce no output
      expect(result).toBe('');
    });
  });

  describe('edge cases', () => {
    it('handles header with complex trailing annotation', () => {
      const testFile = join(testTmpDir, 'complex-annotation.md');
      writeFileSync(
        testFile,
        `# Test

Last updated: 2026-06-01 (PR #625: bridge HMAC handshake)

Content.
`
      );

      runScript([testFile]);

      const content = readFileSync(testFile, 'utf-8');
      expect(content).toContain(`Last updated: ${TEST_DATE} (PR #625: bridge HMAC handshake)`);
    });

    it('handles header on line 1 (unusual but valid)', () => {
      const testFile = join(testTmpDir, 'header-line1.md');
      writeFileSync(
        testFile,
        `Last updated: 2026-06-01

# Title

Content.
`
      );

      runScript([testFile]);

      const content = readFileSync(testFile, 'utf-8');
      expect(content).toContain(`Last updated: ${TEST_DATE}`);
    });

    it('handles date within first 10 lines (max search depth)', () => {
      const testFile = join(testTmpDir, 'header-line9.md');
      // Line 1: # Title
      // Line 2: (empty)
      // Line 3: Subtitle
      // Line 4: (empty)
      // Line 5: Paragraph
      // Line 6: (empty)
      // Line 7: Extra line
      // Line 8: (empty)
      // Line 9: Last updated: 2026-06-01 (this is index 8, within 0-9)
      writeFileSync(
        testFile,
        `# Title

Subtitle

Paragraph

Extra line

Last updated: 2026-06-01

Content.
`
      );

      runScript([testFile]);

      const content = readFileSync(testFile, 'utf-8');
      expect(content).toContain(`Last updated: ${TEST_DATE}`);
    });

    it('ignores date beyond line 10 (outside search depth)', () => {
      const testFile = join(testTmpDir, 'header-line11.md');
      const originalContent = `# Title

Subtitle

Paragraph

Extra line

Another line

Yet another

One more

Last updated: 2026-06-01

Content.
`;
      writeFileSync(testFile, originalContent);

      runScript([testFile]);

      expect(readFileSync(testFile, 'utf-8')).toBe(originalContent);
    });

    it('respects BUMP_DATE env var (not today)', () => {
      const testFile = join(testTmpDir, 'env-var-test.md');
      writeFileSync(
        testFile,
        `# Test

Last updated: 2026-06-01

Content.
`
      );

      // Run with BUMP_DATE set to TEST_DATE
      execFileSync('node', [scriptPath, testFile], {
        env: { ...process.env, BUMP_DATE: TEST_DATE },
      });

      const content = readFileSync(testFile, 'utf-8');
      expect(content).toContain(`Last updated: ${TEST_DATE}`);
      // Verify it's NOT today (even though script defaults to today)
      expect(content).not.toMatch(/Last updated: 2026-07-1[6-9]/);
    });
  });
});
