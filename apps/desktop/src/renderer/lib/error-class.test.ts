import { describe, expect, it } from 'vitest';

import { errorClass, errorDetail } from './error-class';

describe('errorClass', () => {
  it('reduces an Error to its class name, dropping the message', () => {
    const err = new Error(
      'Export blocked: Header link https://linkedin.com/in/jane is not allowed'
    );
    expect(errorClass(err)).toBe('Error');
    expect(errorClass(err)).not.toContain('linkedin');
  });

  it('keeps a subclass name, which is the useful part', () => {
    class ExportError extends Error {
      override name = 'ExportError';
    }
    expect(errorClass(new ExportError('anything'))).toBe('ExportError');
  });

  it('never returns the value itself for a non-Error throw', () => {
    // A rejected string is the other shape these catch blocks see. It must not
    // be echoed — a backend rejection can be a bare message string.
    expect(errorClass('Header link https://example.com/private leaked')).toBe('string');
    expect(errorClass({ url: 'https://example.com/private' })).toBe('object');
    expect(errorClass(undefined)).toBe('undefined');
  });
});

describe('errorDetail', () => {
  it('keeps a provider error intact — this is the whole point', () => {
    // The line that would have diagnosed the macOS report in one look.
    const msg = 'Ollama 500: {"error":"the input length exceeds the context length"}';
    expect(errorDetail(new Error(msg))).toBe(msg);
  });

  it('caps a long message so document text cannot spill wholesale', () => {
    const leaked = `Rejected input: ${'résumé body text '.repeat(200)}`;
    const out = errorDetail(new Error(leaked));
    expect(out.length).toBeLessThanOrEqual(300 + '…[truncated]'.length);
    expect(out).toContain('Rejected input:');
    expect(out.endsWith('…[truncated]')).toBe(true);
  });

  it('marks truncation rather than silently cutting', () => {
    // A silently-cut message reads as a complete provider error that just
    // happens to end oddly — the marker is what tells a reader to distrust it.
    expect(errorDetail(new Error('x'.repeat(301)))).toMatch(/…\[truncated]$/);
    expect(errorDetail(new Error('x'.repeat(300)))).not.toMatch(/truncated/);
  });

  it('handles a non-Error throw without exposing an object dump', () => {
    expect(errorDetail('plain rejection')).toBe('plain rejection');
  });
});
