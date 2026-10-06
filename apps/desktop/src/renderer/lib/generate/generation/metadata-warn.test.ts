import { describe, expect, it, vi } from 'vitest';

import { extractMetadata } from './generation';
import { installGenerationHooks, register } from './test-support';

installGenerationHooks();

// JSON.stringify drops an Error's message, so render it explicitly.
const logged = (calls: unknown[]) =>
  JSON.stringify(calls, (_k, v) => (v instanceof Error ? `${v.name}: ${v.message}` : v));

const SENTINEL = 'PII-SENTINEL jane@example.com';

describe('extractMetadata — failure log carries no raw error message', () => {
  it('warns with the error class, never the message', async () => {
    register({ ai: { generatePipeline: vi.fn().mockRejectedValue(new Error(SENTINEL)) } });
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    await extractMetadata('Jane Smith resume', 'A React role', 'llama3');
    expect(warn).toHaveBeenCalled();
    expect(logged(warn.mock.calls)).not.toContain('PII-SENTINEL');
    expect(logged(warn.mock.calls)).toContain('Error');
  });
});
