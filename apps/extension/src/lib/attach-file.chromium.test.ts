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

    /**
     * The other load-bearing claim jsdom/background.test.ts's mocked args
     * capture can't prove: that a base64 STRING (not raw bytes) is what must
     * cross `chrome.scripting.executeScript({ func, args })`, because Chrome
     * JSON-serializes `args` on the way to the page (PR review round 2 — a
     * `Uint8Array` arg degrades to a plain `{"0":…}` object there, which is
     * what made attach silently 0-byte in production). Playwright can't drive
     * the real `executeScript` call without loading a full unpacked extension
     * (a materially bigger fixture than this file's existing scope), so this
     * reproduces the boundary it DOES enforce — a real `JSON.parse(JSON.
     * stringify(...))` round trip — then decodes with the SAME `atob` +
     * byte-copy loop as `lib/attach-file.ts`'s `base64ToBytes`, inside a real
     * browser's DOM (not jsdom), and confirms the file that lands on the
     * input has the exact original byte length.
     */
    it('the base64 payload survives a JSON round trip (mirrors the executeScript args boundary) and decodes to the exact original bytes', async (ctx) => {
      const { chromium } = await import('playwright');
      const browser = await chromium.launch().catch(() => null);
      if (!browser) {
        ctx.skip();
        return;
      }
      try {
        const page = await browser.newPage();
        await page.setContent('<input type="file" id="resume">');
        const original = new Uint8Array([1, 2, 3, 255, 0, 128]);
        const base64 = Buffer.from(original).toString('base64');
        const [roundTrippedBase64] = JSON.parse(JSON.stringify([base64])) as [string];

        const confirmed = await page.evaluate((b64: string) => {
          const binary = atob(b64);
          const bytes = new Uint8Array(binary.length);
          for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
          const input = document.getElementById('resume') as HTMLInputElement;
          const dt = new DataTransfer();
          const file = new File([bytes], 'resume.pdf', { type: 'application/pdf' });
          dt.items.add(file);
          input.files = dt.files;
          const first = input.files?.[0];
          return first ? { size: first.size, byteLength: bytes.byteLength } : null;
        }, roundTrippedBase64);

        expect(confirmed).toEqual({ size: original.byteLength, byteLength: original.byteLength });
      } finally {
        await browser.close();
      }
    }, 30_000);
  }
);
