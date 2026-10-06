/** Documents into ATS: `documentExportText` (copy/paste) and `documentAttach` (file input). */

import { beforeEach, describe, expect, it } from 'vitest';

import {
  executeScriptMock,
  FAKE_TOKEN,
  getTokenMock,
  mockClient,
  paired,
  resetMocks,
  resumeExport,
  scriptResults,
  send,
  tabSequence,
} from './test-support';

beforeEach(resetMocks);

const APPLY_URL = 'https://example.com/apply';
const attachRequest = {
  kind: 'documentAttach',
  source: { kind: 'document', id: 'doc-1' },
  templateId: 'classic',
  format: 'pdf',
} as const;
const ATTACHED = { attached: true, filename: 'resume.pdf', byteLength: 13 };

describe('documentExportText request', () => {
  it('decodes the base64 cover-letter bytes to UTF-8 text (never leaks base64 to the caller)', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    const encoded = Buffer.from('Dear hiring manager, — Ünïcödé', 'utf8').toString('base64');
    mockClient.documentExport.mockResolvedValue({
      ...resumeExport(encoded),
      mimeType: 'text/plain',
      filename: 'letter.txt',
      kind: 'cover-letter',
      format: 'txt',
    });

    const res = await send({
      kind: 'documentExportText',
      source: { kind: 'document', id: 'doc-1' },
      templateId: 'classic',
      letterLayoutId: 'banded',
    });

    expect(res).toEqual({
      ok: true,
      kind: 'documentExportText',
      text: 'Dear hiring manager, — Ünïcödé',
      filename: 'letter.txt',
    });
    expect(mockClient.documentExport).toHaveBeenCalledWith({
      source: { kind: 'document', id: 'doc-1' },
      kind: 'cover-letter',
      format: 'txt',
      templateId: 'classic',
      letterLayoutId: 'banded',
    });
  });

  it('surfaces a desktop refusal verbatim, never folded away', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.documentExport.mockResolvedValue({ ok: false, error: 'not_found' });

    const res = await send({
      kind: 'documentExportText',
      source: { kind: 'generation', url: 'https://example.com/job/1' },
      templateId: 'classic',
    });

    expect(res).toEqual({ ok: false, error: 'not_found' });
  });
});

describe('documentAttach request', () => {
  it('surfaces a desktop export refusal verbatim, never injects', async () => {
    // The tab+origin are captured BEFORE the export round trip (PR review round
    // 2), so a valid active tab must resolve even on a refusal.
    paired(APPLY_URL);
    mockClient.documentExport.mockResolvedValue({ ok: false, error: 'export_failed' });

    const res = await send(attachRequest);

    expect(res).toEqual({ ok: false, error: 'export_failed' });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });

  it('decodes the base64 bytes, injects attach-file.js, and returns the fail-closed result', async () => {
    paired(APPLY_URL);
    mockClient.documentExport.mockResolvedValue(resumeExport());
    scriptResults(undefined, ATTACHED);

    const res = await send(attachRequest);

    expect(res).toEqual({ ok: true, kind: 'documentAttach', result: ATTACHED });
    expect(executeScriptMock).toHaveBeenCalledTimes(2);
    expect(executeScriptMock.mock.calls[0]?.[0]).toEqual(
      expect.objectContaining({ files: ['attach-file.js'] })
    );
  });

  it('the résumé payload crosses the injection boundary as a JSON-safe base64 string, never a Uint8Array (regression: Chrome JSON-serializes executeScript args, which silently emptied a Uint8Array)', async () => {
    paired(APPLY_URL);
    const encoded = Buffer.from('%PDF-1.4 fake', 'utf8').toString('base64');
    mockClient.documentExport.mockResolvedValue(resumeExport(encoded));
    let capturedFunc: ((...args: unknown[]) => unknown) | undefined;
    let capturedArgs: unknown[] | undefined;
    executeScriptMock.mockResolvedValueOnce([] as never).mockImplementationOnce(async (opts) => {
      const o = opts as { func?: (...args: unknown[]) => unknown; args?: unknown[] };
      capturedFunc = o.func;
      capturedArgs = o.args;
      return [{ result: null }] as never;
    });

    await send(attachRequest);

    // Chrome JSON-serializes `executeScript({ args })` on the way to the page —
    // anything that doesn't survive `JSON.parse(JSON.stringify(...))` never
    // reaches the injected function intact. A `Uint8Array` argument would
    // degrade to a plain `{"0":…}` object here; the base64 STRING survives
    // byte-for-byte.
    const roundTripped = JSON.parse(JSON.stringify(capturedArgs)) as unknown[];
    expect(typeof roundTripped[0]).toBe('string');
    expect(roundTripped[0]).toBe(encoded);

    // And the injected func itself must still resolve to the correct bytes once
    // wired to a real runner — proves the round trip, not just the arg shape.
    const key = roundTripped[3] as string;
    const runnerCalls: unknown[][] = [];
    (globalThis as Record<string, unknown>)[key] = (...args: unknown[]) => {
      runnerCalls.push(args);
      return ATTACHED;
    };
    try {
      const result = capturedFunc?.(...roundTripped);
      expect(result).toEqual(ATTACHED);
      expect(runnerCalls[0]?.[0]).toBe(encoded);
    } finally {
      delete (globalThis as Record<string, unknown>)[key];
    }
  });

  it.each([
    ['the user switches tabs during the export wait', ['https://other.com/page', 9]],
    // Same origin as the confirmed page, so ONLY the tab-id check can refuse.
    [
      'another tab on the same origin becomes active during the export wait',
      ['https://example.com/other-tab', 9],
    ],
    // Same tab id, but navigated to a different origin during the wait.
    [
      'a same-tab navigation to a different origin during the export wait',
      ['https://attacker.example/apply', 7],
    ],
  ] as const)('aborts and never injects when %s', async (_label, [reverified, reverifiedId]) => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.documentExport.mockResolvedValue(resumeExport());
    // Captured at gesture time, origin capture, then the re-verify.
    tabSequence([APPLY_URL], [APPLY_URL], [reverified, reverifiedId]);

    const res = await send(attachRequest);

    expect(res).toEqual({ ok: false, error: 'The page changed while exporting — please retry.' });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });

  it('surfaces "Could not attach the file on this page." when the injected func returns a non-result', async () => {
    paired(APPLY_URL);
    mockClient.documentExport.mockResolvedValue({ ...resumeExport('AAAA'), byteLength: 3 });
    scriptResults(undefined, null);

    const res = await send(attachRequest);

    expect(res).toEqual({ ok: false, error: 'Could not attach the file on this page.' });
  });
});
