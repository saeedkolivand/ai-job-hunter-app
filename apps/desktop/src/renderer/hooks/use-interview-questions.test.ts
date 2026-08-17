import { createElement, type ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import { generateInterviewQuestions, type GenerationMeta } from '@/lib/generate';

import { useInterviewQuestions } from './use-interview-questions';

// Capture the persisted record so we can assert the generated questions + audiences.
const save = vi.fn().mockResolvedValue({ id: 'gen-1', success: true });
vi.mock('@/providers/AppClientProvider', () => ({
  useAppClient: () => ({ aiGenerations: { save } }),
}));

// Keys object only — keep the hook isolated from the real query-client graph.
vi.mock('@/services/query-client', () => ({
  keys: { aiGenerations: { all: ['aiGenerations'] }, autopilot: { all: ['autopilot'] } },
}));

// Stub the generation pipeline. `parseInterviewQuestions` returns a fixed set so we
// can assert it lands on `questions`; `generateInterviewQuestions` is the seam we
// assert the selected audiences flow into.
vi.mock('@/lib/generate', async () => {
  // The REAL locale allowlist + clamp (that module is dependency-free), so the
  // language default is exercised rather than re-implemented by the mock.
  const { safeLocale, OUTPUT_LANGUAGES } = await import('@/lib/generate/locales');
  return {
    safeLocale,
    OUTPUT_LANGUAGES,
    extractMetadata: vi.fn().mockResolvedValue({
      candidateName: 'Ada',
      jobTitle: 'Engineer',
      companyName: 'Acme',
      resumeLanguage: 'en',
      jobAdLanguage: 'en',
      mismatch: false,
      targetLanguage: 'en',
      topRequirements: [],
    }),
    researchCompany: vi.fn().mockResolvedValue('BRIEF'),
    generateInterviewQuestions: vi.fn().mockResolvedValue('RAW'),
    parseInterviewQuestions: vi
      .fn()
      .mockReturnValue([{ id: 'iq-1', question: 'Q1', why: 'because', audience: 'recruiter' }]),
  };
});

// Active provider + Ollama web-search key — controllable so we can exercise the
// needsResearchKey hint (Ollama-family provider without the key), routed through
// the shared `useNeedsResearchKey` hook's own dependencies (both settled, so the
// hook's cold-boot pending guard never suppresses these).
let mockProvider = 'openai';
let mockHasOllamaKey = true;
vi.mock('@/services', () => ({
  useActiveConfig: () => ({ data: { activeProvider: mockProvider }, isPending: false }),
  useHasProviderKey: () => ({ data: { has: mockHasOllamaKey }, isPending: false }),
}));

const params = {
  resume: 'my resume',
  jobDesc: 'JD',
  model: 'llama',
  canUse: true,
  hasDesc: true,
  jobUrl: 'https://acme.com/job/42',
  board: 'linkedin',
};

// The hook uses useQueryClient (to invalidate after save), so it must render under
// a QueryClientProvider.
const wrapper = ({ children }: { children: ReactNode }) =>
  createElement(QueryClientProvider, { client: new QueryClient() }, children);
const render = (p: Parameters<typeof useInterviewQuestions>[0] = params) =>
  renderHook(() => useInterviewQuestions(p), { wrapper });

/** Same, but props-driven so a test can `rerender` with different inputs. */
const renderProps = (initialProps: Parameters<typeof useInterviewQuestions>[0]) =>
  renderHook((p: Parameters<typeof useInterviewQuestions>[0]) => useInterviewQuestions(p), {
    wrapper,
    initialProps,
  });

/** Long enough for the real `detectLanguage` heuristic to settle on German. */
const GERMAN_JD =
  'Wir suchen einen erfahrenen Softwareentwickler für unser Team in München. ' +
  'Sie arbeiten an spannenden Projekten und stimmen sich eng mit dem Produktteam ab.';

/** Dutch — detectable by franc, but NOT one of the picker's OUTPUT_LANGUAGES. */
const DUTCH_JD =
  'Wij zoeken een ervaren softwareontwikkelaar voor ons team in Amsterdam. ' +
  'Je werkt aan uitdagende projecten en stemt nauw af met het productteam.';

const metaWithLanguage = (targetLanguage: string): GenerationMeta => ({
  candidateName: 'Ada',
  jobTitle: 'Engineer',
  companyName: 'Acme',
  resumeLanguage: 'en',
  jobAdLanguage: targetLanguage,
  mismatch: false,
  targetLanguage,
  topRequirements: [],
});

describe('useInterviewQuestions', () => {
  beforeEach(() => {
    save.mockClear();
    vi.mocked(generateInterviewQuestions).mockClear();
    mockProvider = 'openai';
    mockHasOllamaKey = true;
  });

  it('defaults to the two earliest rounds (recruiter/HR + hiring manager)', () => {
    const { result } = render();
    expect(result.current.audiences).toEqual(['recruiter', 'hiringManager']);
  });

  it('toggleAudience adds an unselected audience and removes a selected one', () => {
    const { result } = render();

    act(() => result.current.toggleAudience('team'));
    expect(result.current.audiences).toEqual(['recruiter', 'hiringManager', 'team']);

    act(() => result.current.toggleAudience('recruiter'));
    expect(result.current.audiences).toEqual(['hiringManager', 'team']);
  });

  it('canGenerate is false once every audience is deselected (even with valid inputs)', () => {
    const { result } = render();
    expect(result.current.canGenerate).toBe(true);

    act(() => result.current.toggleAudience('recruiter'));
    act(() => result.current.toggleAudience('hiringManager'));

    expect(result.current.audiences).toEqual([]);
    expect(result.current.canGenerate).toBe(false);
  });

  it('canGenerate is false when AI is unavailable, no job description, or empty résumé', () => {
    expect(render({ ...params, canUse: false }).result.current.canGenerate).toBe(false);
    expect(render({ ...params, hasDesc: false }).result.current.canGenerate).toBe(false);
    expect(render({ ...params, resume: '   ' }).result.current.canGenerate).toBe(false);
  });

  it('passes the selected audiences to generateInterviewQuestions and persists the parsed set', async () => {
    const { result } = render();

    act(() => result.current.toggleAudience('team')); // recruiter, hiringManager, team

    await act(async () => {
      await result.current.generate();
    });

    expect(generateInterviewQuestions).toHaveBeenCalledTimes(1);
    expect(generateInterviewQuestions).toHaveBeenCalledWith(
      expect.objectContaining({ audiences: ['recruiter', 'hiringManager', 'team'] })
    );
    // Parsed questions land on state and are persisted onto the per-job record.
    expect(result.current.questions).toHaveLength(1);
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        jobUrl: 'https://acme.com/job/42',
        interviewQuestions: [{ id: 'iq-1', question: 'Q1', why: 'because', audience: 'recruiter' }],
      })
    );
  });

  it('flags needsResearchKey for an Ollama provider missing the web-search key', () => {
    mockProvider = 'ollama';
    mockHasOllamaKey = false;
    expect(render().result.current.needsResearchKey).toBe(true);
  });

  it('does not flag needsResearchKey for a cloud provider (uses its own web search)', () => {
    mockProvider = 'openai';
    mockHasOllamaKey = false;
    expect(render().result.current.needsResearchKey).toBe(false);
  });

  it('does not flag needsResearchKey once the Ollama key is present', () => {
    mockProvider = 'ollama';
    mockHasOllamaKey = true;
    expect(render().result.current.needsResearchKey).toBe(false);
  });

  it('defaults the output language to the one detected from the job description', () => {
    expect(render({ ...params, jobDesc: GERMAN_JD }).result.current.language).toBe('de');
  });

  it('prefers an already-detected meta.targetLanguage over re-detecting the ad', () => {
    // German ad text, but the tailor flow already resolved the target as French.
    const { result } = render({ ...params, jobDesc: GERMAN_JD, meta: metaWithLanguage('fr') });
    expect(result.current.language).toBe('fr');
  });

  it('keeps a language outside OUTPUT_LANGUAGES instead of clamping it to English', () => {
    // The picker renders it as an extra option; clamping here would make the UI
    // claim "English" while generation produced Dutch.
    const { result } = render({ ...params, meta: metaWithLanguage('nl') });
    expect(result.current.language).toBe('nl');
    expect(result.current.detectedLanguage).toBe('nl');
  });

  it('passes the chosen language to the generator WITHOUT writing it to the shared record', async () => {
    // Ad is German, so the default is 'de' — the explicit pick must win over it.
    const { result } = render({ ...params, jobDesc: GERMAN_JD });
    expect(result.current.language).toBe('de');

    act(() => result.current.setLanguage('es'));
    expect(result.current.language).toBe('es');

    await act(async () => {
      await result.current.generate();
    });

    expect(generateInterviewQuestions).toHaveBeenCalledWith(
      expect.objectContaining({ language: 'es' })
    );
    // `targetLanguage` is SHARED with the email tab / export meta / tailor seed,
    // so the picker must never write to it — the extracted value stands.
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ targetLanguage: 'en' }));
    expect(save).not.toHaveBeenCalledWith(expect.objectContaining({ targetLanguage: 'es' }));
  });

  it('leaves the record targetLanguage alone on the untouched default path too', async () => {
    // franc says 'de' (the ad), extractMetadata says 'en' — they disagree, and
    // the save must still carry the EXTRACTED value while generation uses the ad's.
    const { result } = render({ ...params, jobDesc: GERMAN_JD });

    await act(async () => {
      await result.current.generate();
    });

    expect(generateInterviewQuestions).toHaveBeenCalledWith(
      expect.objectContaining({ language: 'de' })
    );
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ targetLanguage: 'en' }));
  });

  describe('unconfident target language is never persisted', () => {
    it('sends empty language fields when meta was seeded from an unconfident guess', async () => {
      const { result } = render({
        ...params,
        meta: metaWithLanguage('en'),
        targetLanguageConfident: false,
      });

      await act(async () => {
        await result.current.generate();
      });

      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({ targetLanguage: '', resumeLanguage: '', jobAdLanguage: '' })
      );
    });

    it('still persists the real language fields when meta was confidently detected', async () => {
      const { result } = render({
        ...params,
        meta: metaWithLanguage('en'),
        targetLanguageConfident: true,
      });

      await act(async () => {
        await result.current.generate();
      });

      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({ targetLanguage: 'en', resumeLanguage: 'en', jobAdLanguage: 'en' })
      );
    });

    it('an unconfident flag with no meta at all is a no-op (extractMetadata path is unrelated)', async () => {
      // No `meta` → generate() re-extracts fresh; a stale/irrelevant confidence
      // flag from a caller that never passed `meta` must not blank the record.
      const { result } = render({ ...params, targetLanguageConfident: false });

      await act(async () => {
        await result.current.generate();
      });

      expect(save).toHaveBeenCalledWith(expect.objectContaining({ targetLanguage: 'en' }));
    });
  });

  it('generates in a detected language the picker cannot name, and says so', async () => {
    const { result } = render({ ...params, jobDesc: DUTCH_JD });
    // What the picker displays IS what generates — no silent "English".
    expect(result.current.language).toBe('nl');
    expect(result.current.detectedLanguage).toBe('nl');

    await act(async () => {
      await result.current.generate();
    });

    expect(generateInterviewQuestions).toHaveBeenCalledWith(
      expect.objectContaining({ language: 'nl' })
    );
  });

  it('omits the language entirely when nothing can be detected (prompt falls back to meta)', async () => {
    // 'JD' is below detectLanguage's length floor → 'unknown' → '' (the picker's
    // "match the job ad" slot), and no language on the request.
    const { result } = render();
    expect(result.current.language).toBe('');

    await act(async () => {
      await result.current.generate();
    });

    expect(generateInterviewQuestions).toHaveBeenCalledWith(
      expect.objectContaining({ language: undefined })
    );
  });

  it('accepts a language NAME from extractMetadata fallback as the picker default', () => {
    // extractMetadata's regex fallback can yield 'German' rather than 'de'.
    expect(render({ ...params, meta: metaWithLanguage('German') }).result.current.language).toBe(
      'de'
    );
  });

  it('keeps an explicit pick across a jobDesc/meta change (the default must not win back)', () => {
    const { result, rerender } = renderProps({ ...params, jobDesc: GERMAN_JD });
    expect(result.current.language).toBe('de');

    act(() => result.current.setLanguage('it'));
    expect(result.current.language).toBe('it');

    // The ad is replaced (and metadata arrives) — the derived default would now
    // say 'nl'/'fr', but the user's choice owns the value from here on.
    rerender({ ...params, jobDesc: DUTCH_JD, meta: metaWithLanguage('fr') });

    expect(result.current.language).toBe('it');
    expect(result.current.detectedLanguage).toBe('fr');
  });

  describe('generate() failure handling', () => {
    it('surfaces the error message and clears the generating flag', async () => {
      vi.mocked(generateInterviewQuestions).mockRejectedValueOnce(new Error('model exploded'));
      const { result } = render();

      await act(async () => {
        await result.current.generate();
      });

      expect(result.current.error).toBe('model exploded');
      expect(result.current.generating).toBe(false);
      // A failed run must not half-populate the UI or persist anything.
      expect(result.current.questions).toEqual([]);
      expect(save).not.toHaveBeenCalled();
    });

    it('falls back to a fixed message when the rejection is not an Error', async () => {
      vi.mocked(generateInterviewQuestions).mockRejectedValueOnce('just a string');
      const { result } = render();

      await act(async () => {
        await result.current.generate();
      });

      expect(result.current.error).toBe('Failed to generate interview questions');
      expect(result.current.generating).toBe(false);
    });

    it('clears a previous error when a later run succeeds', async () => {
      vi.mocked(generateInterviewQuestions).mockRejectedValueOnce(new Error('transient'));
      const { result } = render();

      await act(async () => {
        await result.current.generate();
      });
      expect(result.current.error).toBe('transient');

      await act(async () => {
        await result.current.generate();
      });

      expect(result.current.error).toBeNull();
      expect(result.current.questions).toHaveLength(1);
    });

    it('reports a failure raised by the persistence step, not just the generator', async () => {
      save.mockRejectedValueOnce(new Error('disk full'));
      const { result } = render();

      await act(async () => {
        await result.current.generate();
      });

      expect(result.current.error).toBe('disk full');
      expect(result.current.generating).toBe(false);
      // The questions still parsed, so the UI keeps them despite the save failing.
      expect(result.current.questions).toHaveLength(1);
    });
  });

  it('does not call the generator when no audience is selected', async () => {
    const { result } = render();

    act(() => result.current.toggleAudience('recruiter'));
    act(() => result.current.toggleAudience('hiringManager'));

    await act(async () => {
      await result.current.generate();
    });

    expect(generateInterviewQuestions).not.toHaveBeenCalled();
    expect(save).not.toHaveBeenCalled();
  });
});
