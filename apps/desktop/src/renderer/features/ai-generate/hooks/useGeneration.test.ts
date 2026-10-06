// useGeneration — progressive reveal, single target, URL-import provenance.
// Quality report + stale-persist races → `useGeneration.quality.test.ts`.
import { describe, expect, it, vi } from 'vitest';

import { generateCoverLetter, generateResume } from '@/lib/generate';

import { installGenerationMocks, runGeneration, stageCalls } from './useGeneration.test-support';

vi.mock('@/lib/generate', async () => (await import('./useGeneration.stubs')).generateMock());

installGenerationMocks();

describe('useGeneration — progressive reveal (#23)', () => {
  it('reveals the résumé (stage done) and finishes both, with a success toast', async () => {
    const { m } = await runGeneration('both');

    const stages = stageCalls(m);
    expect(stages[0]).toBe('generating');
    // 'done' is set twice: once right after the résumé (progressive reveal) and
    // again at the end — the double-flip is the reveal signature.
    expect(stages.filter((s) => s === 'done').length).toBeGreaterThanOrEqual(2);
    expect(stages.at(-1)).toBe('done');

    expect(m.setIsGenerating).toHaveBeenCalledWith(true);
    expect(m.setIsGenerating).toHaveBeenLastCalledWith(false);
    expect(m.notify.success).toHaveBeenCalledWith({ message: 'aiGenerate.toast.bothReady' });
    // The cover-letter research brief is persisted alongside the documents.
    expect(m.saveAiGeneration.mutate).toHaveBeenCalledWith(
      expect.objectContaining({
        resumeText: 'RESUME',
        coverLetterText: 'COVER',
        companyBrief: 'BRIEF',
      })
    );
    expect(m.setError).not.toHaveBeenCalledWith(expect.any(String));
  });

  it('keeps the finished résumé when the cover letter fails, and flags it', async () => {
    vi.mocked(generateCoverLetter).mockRejectedValueOnce(new Error('cover boom'));
    const { m } = await runGeneration('both');

    // The résumé is salvaged: we end on 'done', never bouncing back to configuring.
    expect(stageCalls(m).at(-1)).toBe('done');
    expect(stageCalls(m)).not.toContain('configuring');
    expect(m.notify.error).toHaveBeenCalledWith({ message: 'aiGenerate.toast.coverFailed' });
    // Persisted résumé-only (no cover text / no brief), and no hard error surfaced.
    expect(m.saveAiGeneration.mutate).toHaveBeenCalledWith(
      expect.objectContaining({ resumeText: 'RESUME', coverLetterText: '', companyBrief: '' })
    );
    expect(m.setError).not.toHaveBeenCalledWith(expect.any(String));
    expect(m.setIsGenerating).toHaveBeenLastCalledWith(false);
  });

  it('surfaces a hard error and returns to configuring when the résumé fails', async () => {
    vi.mocked(generateResume).mockRejectedValueOnce(new Error('resume boom'));
    const { m } = await runGeneration('both');

    expect(stageCalls(m).at(-1)).toBe('configuring');
    expect(m.setError).toHaveBeenCalledWith('resume boom');
    expect(m.notify.error).toHaveBeenCalledWith({ message: 'aiGenerate.toast.failed' });
    expect(m.saveAiGeneration.mutate).not.toHaveBeenCalled();
    expect(m.setIsGenerating).toHaveBeenLastCalledWith(false);
  });
});

describe('useGeneration — single target', () => {
  it('cover-only stays in the streaming view until done, then notifies', async () => {
    const { m } = await runGeneration('cover');

    const stages = stageCalls(m);
    // No early progressive 'done' for a single document — only the final one.
    expect(stages).toEqual(['generating', 'done']);
    expect(generateResume).not.toHaveBeenCalled();
    expect(m.notify.success).toHaveBeenCalledWith({ message: 'aiGenerate.toast.coverReady' });
    expect(m.saveAiGeneration.mutate).toHaveBeenCalledWith(
      expect.objectContaining({ resumeText: '', coverLetterText: 'COVER', companyBrief: 'BRIEF' })
    );
  });

  it('resume-only generates just the résumé and notifies', async () => {
    const { m } = await runGeneration('resume');

    expect(stageCalls(m)).toEqual(['generating', 'done']);
    expect(generateCoverLetter).not.toHaveBeenCalled();
    expect(m.notify.success).toHaveBeenCalledWith({ message: 'aiGenerate.toast.resumeReady' });
  });
});

describe('useGeneration — URL-import provenance (ADR-031)', () => {
  it('persists jobUrl + board when the ad came from a URL import', async () => {
    const { m } = await runGeneration('resume', {
      jobUrl: 'https://boards.greenhouse.io/acme/jobs/1',
      board: 'greenhouse',
    });

    expect(m.saveAiGeneration.mutate).toHaveBeenCalledWith(
      expect.objectContaining({
        jobUrl: 'https://boards.greenhouse.io/acme/jobs/1',
        board: 'greenhouse',
      })
    );
  });

  it('omits jobUrl + board for pasted text (never invents provenance)', async () => {
    const { m } = await runGeneration('resume');

    expect(m.saveAiGeneration.mutate).toHaveBeenCalled();
    // No call carries provenance keys when the ad wasn't URL-imported.
    expect(m.saveAiGeneration.mutate).not.toHaveBeenCalledWith(
      expect.objectContaining({ jobUrl: expect.anything() })
    );
    expect(m.saveAiGeneration.mutate).not.toHaveBeenCalledWith(
      expect.objectContaining({ board: expect.anything() })
    );
  });
});
