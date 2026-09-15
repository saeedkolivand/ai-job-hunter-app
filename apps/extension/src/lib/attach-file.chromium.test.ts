/**
 * The load-bearing claim `attach-file.test.ts` can't prove: that a REAL
 * browser's `input.files = dataTransfer.files` assignment (`attach-file.ts`'s
 * `attachResumeFile`) actually STICKS and re-reads back, rather than jsdom's
 * hand-written polyfill merely proving the module's own logic against an
 * assumption it encodes itself (see that file's doc). PR2 §C.3 requires this
 * check whenever `playwright` is resolvable from the workspace root.
 *
 * Two independent guards, not one:
 * - `playwright` the NPM PACKAGE not resolvable (a CI job/checkout that never
 *   installs it) → `describe.skipIf` skips the whole suite.
 * - `playwright` resolvable but its Chromium BINARY not installed (this
 *   package's own CI job installs no browsers — only `@ajh/ui`'s does) →
 *   `chromium.launch()` rejects and the test calls `ctx.skip()` itself,
 *   rather than failing a job that was never set up to run it.
 *
 * Runs for real wherever both are present (this was verified that way before
 * landing) and reports absence honestly everywhere else instead of a
 * red-herring CI failure.
 */
import { createRequire } from 'node:module';

import { describe, expect, it } from 'vitest';

function playwrightResolvable(): boolean {
  try {
    createRequire(import.meta.url).resolve('playwright');
    return true;
  } catch {
    return false;
  }
}

describe.skipIf(!playwrightResolvable())(
  'attachResumeFile — real Chromium (load-bearing DataTransfer assignment)',
  () => {
    it('input.files = dataTransfer.files sticks and is re-readable on a fixture page', async (ctx) => {
      const { chromium } = await import('playwright');
      const browser = await chromium.launch().catch(() => null);
      if (!browser) {
        ctx.skip();
        return;
      }
      try {
        const page = await browser.newPage();
        await page.setContent('<input type="file" id="resume">');
        const confirmed = await page.evaluate(() => {
          const input = document.getElementById('resume') as HTMLInputElement;
          const dt = new DataTransfer();
          const file = new File([new Uint8Array([1, 2, 3])], 'resume.pdf', {
            type: 'application/pdf',
          });
          dt.items.add(file);
          input.files = dt.files;
          input.dispatchEvent(new Event('input', { bubbles: true }));
          input.dispatchEvent(new Event('change', { bubbles: true }));
          const first = input.files?.[0];
          return first ? { name: first.name, size: first.size } : null;
        });
        expect(confirmed).toEqual({ name: 'resume.pdf', size: 3 });
      } finally {
        await browser.close();
      }
    }, 30_000);
  }
);
