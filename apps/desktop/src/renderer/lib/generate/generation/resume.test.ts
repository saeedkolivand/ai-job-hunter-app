import { describe, expect, it, vi } from 'vitest';

import { generateResume, synthesizeResume } from './generation';
import {
  done,
  emit,
  flushUntilStreaming,
  installGenerationHooks,
  register,
  registerWithContactProfile,
  streamThrough,
} from './test-support';

installGenerationHooks();

describe('generateResume', () => {
  it('streams content, strips <think> blocks, and forwards tokens', async () => {
    register();
    const onToken = vi.fn();
    const onThinking = vi.fn();
    const out = await streamThrough(
      generateResume(
        'My resume',
        'Job ad',
        {
          resumeLanguage: 'en',
          jobAdLanguage: 'en',
          mismatch: false,
          candidateName: 'X',
          jobTitle: 'Y',
          companyName: 'Z',
          targetLanguage: 'en',
          topRequirements: [],
        },
        'ats',
        'llama3',
        onToken,
        'en',
        undefined,
        onThinking
      ),
      '<think>reasoning here</think>VISIBLE RESUME CONTENT'
    );
    expect(out).toContain('VISIBLE RESUME CONTENT');
    expect(out).not.toContain('reasoning here');
    expect(onThinking).toHaveBeenCalled();
    expect(onToken).toHaveBeenCalled();
  });

  const RESUME_META = {
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    candidateName: 'X',
    jobTitle: 'Y',
    companyName: 'Z',
    targetLanguage: 'en',
    topRequirements: [],
  };

  // H — the editor is the source of truth: the header seeded from the Contact
  // Profile after generation (see generation.ts `seedHeaderFromProfile`).
  it('seeds the header from a non-empty contact profile after generation', async () => {
    registerWithContactProfile({
      get: vi.fn().mockResolvedValue({
        fullName: 'Jordan Lee',
        email: 'jordan@example.com',
      }),
      headerLine: vi.fn().mockResolvedValue('Berlin | jordan@example.com'),
    });
    const out = await streamThrough(
      generateResume(
        'My resume',
        'Job ad',
        RESUME_META,
        'ats',
        'llama3',
        vi.fn(),
        'en',
        undefined,
        undefined
      ),
      'Model Name\nmodel@example.com | +1 555 0100\n\nSUMMARY\nModel-written summary.'
    );
    expect(out).toContain('Jordan Lee');
    expect(out).toContain('Berlin | jordan@example.com');
    expect(out).not.toContain('model@example.com');
    // Body content survives the header rewrite.
    expect(out).toContain('Model-written summary.');
  });

  it('inserts a contact line when the model generated none', async () => {
    registerWithContactProfile({
      get: vi.fn().mockResolvedValue({ email: 'jordan@example.com' }),
      headerLine: vi.fn().mockResolvedValue('jordan@example.com'),
    });
    const out = await streamThrough(
      generateResume(
        'My resume',
        'Job ad',
        RESUME_META,
        'ats',
        'llama3',
        vi.fn(),
        'en',
        undefined,
        undefined
      ),
      'Model Name\n\nSUMMARY\nNo contact line at all.'
    );
    expect(out.split('\n')[1]).toBe('jordan@example.com');
  });

  // LOW (security re-review): the post-stream header-seeding IPC calls now
  // honor the generation AbortSignal — a cancel that lands right as the
  // stream finishes (before the seeding step runs) must skip it entirely
  // (the model's own header survives untouched), not silently apply a
  // seeding patch to a result the caller has already discarded. Aborted
  // AFTER `done()` (not before) — `awaitAiStream` already rejects upfront
  // for a pre-aborted signal (a separate, already-covered guard), so this
  // isolates the seeding-step check specifically: the stream itself must
  // still resolve successfully, with cancellation landing exactly in the
  // gap between stream completion and the seeding IPC calls.
  it('does not seed the header when the generation is cancelled right as the stream finishes', async () => {
    const get = vi.fn().mockResolvedValue({ fullName: 'Jordan Lee', email: 'jordan@example.com' });
    const headerLine = vi.fn().mockResolvedValue('Berlin | jordan@example.com');
    registerWithContactProfile({ get, headerLine });
    const controller = new AbortController();
    const p = generateResume(
      'My resume',
      'Job ad',
      RESUME_META,
      'ats',
      'llama3',
      vi.fn(),
      'en',
      controller.signal,
      undefined
    );
    await flushUntilStreaming();
    emit('Model Name\nmodel@example.com | +1 555 0100\n\nSUMMARY\nModel-written summary.');
    done();
    controller.abort();
    const out = await p;
    expect(out).toBe(
      'Model Name\nmodel@example.com | +1 555 0100\n\nSUMMARY\nModel-written summary.'
    );
    expect(get).not.toHaveBeenCalled();
    expect(headerLine).not.toHaveBeenCalled();
  });

  // CodeRabbit (test-coverage re-review): the test above only exercises the
  // PRE-CALL guard (`get`/`headerLine` are never invoked at all — the signal
  // is already aborted when seeding starts). The SECOND guard —
  // `if (signal?.aborted) return text;` AFTER the `Promise.all` resolves,
  // before `seedHeaderFromProfile` is applied — had no coverage. Here the
  // signal is NOT aborted when seeding starts (so both IPC calls genuinely
  // fire and resolve), and only becomes aborted WHILE they're in flight (via
  // a side effect inside the `get` mock) — proving the patch is still
  // discarded even though the calls themselves completed successfully.
  it('does not apply the seeding patch when the signal aborts while the post-stream IPC calls are in flight', async () => {
    const controller = new AbortController();
    const get = vi.fn().mockImplementation(async () => {
      // Cancellation lands here — after this call starts, before Promise.all
      // (and therefore the post-resolve guard) settles.
      controller.abort();
      return { fullName: 'Jordan Lee', email: 'jordan@example.com' };
    });
    const headerLine = vi.fn().mockResolvedValue('Berlin | jordan@example.com');
    registerWithContactProfile({ get, headerLine });
    const out = await streamThrough(
      generateResume(
        'My resume',
        'Job ad',
        RESUME_META,
        'ats',
        'llama3',
        vi.fn(),
        'en',
        controller.signal,
        undefined
      ),
      'Model Name\nmodel@example.com | +1 555 0100\n\nSUMMARY\nModel-written summary.'
    );
    // Both calls genuinely fired (the pre-call guard did not block them)...
    expect(get).toHaveBeenCalled();
    expect(headerLine).toHaveBeenCalled();
    // ...but the resolved profile/header-line was never applied — the
    // model's own header survives untouched.
    expect(out).toBe(
      'Model Name\nmodel@example.com | +1 555 0100\n\nSUMMARY\nModel-written summary.'
    );
  });

  it('leaves the model header untouched when the contact profile is effectively empty', async () => {
    register();
    const out = await streamThrough(
      generateResume(
        'My resume',
        'Job ad',
        RESUME_META,
        'ats',
        'llama3',
        vi.fn(),
        'en',
        undefined,
        undefined
      ),
      'Original Name\noriginal@example.com | +1 555 0100'
    );
    expect(out).toBe('Original Name\noriginal@example.com | +1 555 0100');
  });

  // Security review (2026-08): seeding runs AFTER injectLinksIntoGeneratedText,
  // so its safety depends on isHeaderContactLine correctly recognising the
  // line link-injection just wrote (a bare "Website" label turned into a
  // markdown link), not just an email/phone-shaped line.
  it('recognizes and replaces a contact line link-injection just wrote (URL keyword only, no email/phone)', async () => {
    registerWithContactProfile({
      get: vi.fn().mockResolvedValue({
        fullName: 'Jordan Lee',
        website: 'https://jordan.example.com',
      }),
      headerLine: vi.fn().mockResolvedValue('[Website](https://jordan.example.com)'),
    });
    // The source résumé's reference block seeds getLinkMap() with a
    // Website-labelled contact link the model is expected to write as the
    // bare label "Website" — injectLinksIntoGeneratedText() turns that into
    // a markdown link before seedHeaderFromProfile ever runs.
    const sourceResume = 'Some old resume content.\n---\n- [Website](https://real.example.com)';
    const p = generateResume(
      sourceResume,
      'Job ad',
      RESUME_META,
      'ats',
      'llama3',
      vi.fn(),
      'en',
      undefined,
      undefined
    );
    await flushUntilStreaming();
    // No email/phone anywhere — pipe + the "Website" label is the only signal.
    emit('Model Name\nModel City | Website\n\nSUMMARY\nSome text.');
    done();
    const out = await p;
    const lines = out.split('\n');
    expect(lines[1]).toBe('[Website](https://jordan.example.com)');
    expect(out).not.toContain('real.example.com');
  });

  // Security re-review (HIGH, round 4): header seeding is cosmetic
  // post-processing on an already-finished, already-paid-for AI generation —
  // a transient IPC failure here must degrade to "seed nothing," never throw
  // and discard the whole result the caller is about to persist.
  it('does not throw when the contactProfile IPC calls reject — returns the unseeded text', async () => {
    registerWithContactProfile({
      get: vi.fn().mockRejectedValue(new Error('IPC unavailable')),
      headerLine: vi.fn().mockRejectedValue(new Error('IPC unavailable')),
    });
    const out = await streamThrough(
      generateResume(
        'My resume',
        'Job ad',
        RESUME_META,
        'ats',
        'llama3',
        vi.fn(),
        'en',
        undefined,
        undefined
      ),
      'Model Name\nmodel@example.com | +1 555 0100'
    );
    expect(out).toBe('Model Name\nmodel@example.com | +1 555 0100');
  });
});

describe('synthesizeResume', () => {
  const BUILDER_META = {
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    candidateName: 'X',
    jobTitle: 'Y',
    companyName: 'Z',
    targetLanguage: 'en',
    topRequirements: [],
  };
  const ANSWERS = { fullName: 'Jordan Lee', experience: [], education: [], skills: [] };

  // Security re-review (CRITICAL): synthesizeResume (the Resume Builder) had
  // NO seeding call at all — `seedHeaderFromProfile` had exactly one
  // production caller (`generateResume`). The builder prompt has the model
  // write an ordinary name + contact line, same as any other résumé prompt;
  // only this call overwrites it with the profile's own values.
  it('seeds the header from the contact profile — the CRITICAL repro (the Resume Builder path was never seeded)', async () => {
    registerWithContactProfile({
      get: vi.fn().mockResolvedValue({ fullName: 'Jordan Lee', email: 'jordan@example.com' }),
      headerLine: vi.fn().mockResolvedValue('Berlin | jordan@example.com'),
    });
    const out = await streamThrough(
      synthesizeResume(ANSWERS, BUILDER_META, 'llama3', vi.fn(), 'en', undefined, undefined),
      'Some AI Written Name\nmodel@example.com | +1 555 0100\n\nSUMMARY\nBody.'
    );
    expect(out).toContain('Jordan Lee');
    expect(out).toContain('Berlin | jordan@example.com');
    expect(out).not.toContain('model@example.com');
  });
});
