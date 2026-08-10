import { createElement, type ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';

import { computeQualityReport, exportPDF, hashText } from '@/lib/generate';
import type * as QualityReportModule from '@/lib/generate/quality-report';
import { useGenerationStore } from '@/store/generation-store';

import { useTailorGeneration } from './useTailorGeneration';

// i18n: identity translator so phaseLabel resolves without the i18n runtime.
vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

// useNotification: return an API of spies; the export-failure test asserts on it.
const mockNotify = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};
vi.mock('@ajh/ui', () => ({ useNotification: () => mockNotify }));

// Capture the saved application record so we can assert the job link is attached.
// `resume.validateContent` backs the quality panel's Re-check (below).
const save = vi.fn().mockResolvedValue({ id: 'gen-1', success: true });
const validateContent = vi.fn();
vi.mock('@/providers/AppClientProvider', () => ({
  useAppClient: () => ({ aiGenerations: { save }, resume: { validateContent } }),
}));

// Stub the generation pipeline. Resume/cover stubs emit a reasoning token via
// the `onThinking` callback (last arg) so we can assert it threads through.
// The quality-report helpers stay REAL: the Re-check test below asserts on the
// wrapper that actually gets persisted, so a stubbed merge would prove nothing.
vi.mock('@/lib/generate', async () => {
  const quality = await vi.importActual<typeof QualityReportModule>(
    '@/lib/generate/quality-report'
  );
  return {
    hashText: quality.hashText,
    mergeRecheckedReport: quality.mergeRecheckedReport,
    serializeQualityReport: quality.serializeQualityReport,
    ...pipelineStubs,
  };
});

const pipelineStubs = vi.hoisted(() => ({
  extractMetadata: vi.fn().mockResolvedValue({
    candidateName: '',
    jobTitle: '',
    companyName: '',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    targetLanguage: 'en',
    topRequirements: [],
  }),
  generateResume: vi.fn(
    async (
      _resume: string,
      _jobAd: string,
      _meta: unknown,
      _mode: string,
      _model: string,
      onToken: (t: string) => void,
      _locale?: string,
      _signal?: AbortSignal,
      onThinking?: (t: string) => void
    ) => {
      onThinking?.('R-think');
      onToken('R');
      return 'RESUME';
    }
  ),
  generateCoverLetter: vi.fn(
    async (
      _resume: string,
      _jobAd: string,
      _meta: unknown,
      _mode: string,
      _model: string,
      onToken: (t: string) => void,
      _locale?: string,
      _signal?: AbortSignal,
      onThinking?: (t: string) => void
    ) => {
      onThinking?.('C-think');
      onToken('C');
      return { text: 'COVER', companyBrief: 'BRIEF' };
    }
  ),
  buildFilename: vi.fn(),
  exportDOCX: vi.fn(),
  exportPDF: vi.fn(),
  exportTXT: vi.fn(),
  computeQualityReport: vi.fn().mockResolvedValue(null),
}));

const EMPTY_METRICS = {
  keywordCoverage: null,
  topRequirementHits: 0,
  duplicateRatio: 0,
  rolesSource: 0,
  rolesOutput: 0,
};

const params = {
  contextId: 'autopilot:test-job',
  jobDesc: 'JD',
  sourceResume: 'my resume',
  model: 'llama',
  canUse: true,
  hasDesc: true,
  jobUrl: 'https://acme.com/job/42',
  board: 'linkedin',
  researchCompany: false,
  templateId: 'classic' as const,
  atsMode: false,
};

// useTailorGeneration uses useQueryClient (to invalidate after save), so the hook
// must render under a QueryClientProvider.
const wrapper = ({ children }: { children: ReactNode }) =>
  createElement(QueryClientProvider, { client: new QueryClient() }, children);
const render = (p = params) => renderHook(() => useTailorGeneration(p), { wrapper });

describe('useTailorGeneration', () => {
  // The session lives in the shared store — reset it so tests don't leak state.
  beforeEach(() => {
    useGenerationStore.setState({ sessions: {} });
    save.mockClear();
    validateContent.mockReset();
    mockNotify.error.mockClear();
    vi.mocked(computeQualityReport).mockResolvedValue(null);
  });

  it('starts idle with empty buffers', () => {
    const { result } = render();
    expect(result.current.phase).toBe('idle');
    expect(result.current.resumeOut).toBe('');
    expect(result.current.coverOut).toBe('');
    expect(result.current.thinking).toBe('');
  });

  it('threads reasoning into `thinking`, clearing it between phases', async () => {
    const { result } = render();

    await act(async () => {
      await result.current.generate('my resume', 'both');
    });

    expect(result.current.resumeOut).toBe('RESUME');
    expect(result.current.coverOut).toBe('COVER');
    // Cleared at the cover phase, so only the cover-letter reasoning remains.
    expect(result.current.thinking).toBe('C-think');
    expect(result.current.phase).toBe('idle');
  });

  it('saves the application linked to the job url + board after a clean run', async () => {
    const { result } = render();

    await act(async () => {
      await result.current.generate('my resume', 'both');
    });

    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        jobUrl: 'https://acme.com/job/42',
        board: 'linkedin',
        jobAd: 'JD',
        resumeText: 'RESUME',
        coverLetterText: 'COVER',
        mode: 'ats',
        // The cover-letter research brief is persisted so the doc card shows it.
        companyBrief: 'BRIEF',
      })
    );
  });

  it('carries a fresh quality report on the save and exposes it from the hook', async () => {
    const report = {
      schemaVersion: 2 as const,
      pipeline: 'fast' as const,
      generatedAt: 1,
      resume: { report: { ok: true, issues: [], metrics: EMPTY_METRICS }, sourceTextHash: 1 },
      coverLetter: { report: { ok: true, issues: [], metrics: EMPTY_METRICS }, sourceTextHash: 2 },
    };
    vi.mocked(computeQualityReport).mockResolvedValueOnce(report);
    const { result } = render();

    await act(async () => {
      await result.current.generate('my resume', 'both');
    });

    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({ qualityReport: JSON.stringify(report) })
    );
    expect(result.current.report).toEqual(report);
  });

  it('degrades to a report-less save when validation yields nothing, never blocking it', async () => {
    vi.mocked(computeQualityReport).mockResolvedValueOnce(null);
    const { result } = render();

    await act(async () => {
      await result.current.generate('my resume', 'both');
    });

    expect(save).toHaveBeenCalledTimes(1);
    expect(save).not.toHaveBeenCalledWith(
      expect.objectContaining({ qualityReport: expect.anything() })
    );
    expect(result.current.report).toBeNull();
  });

  it('does not generate or save when AI is unavailable', async () => {
    const { result } = render({ ...params, canUse: false });
    await act(async () => {
      await result.current.generate('my resume', 'resume');
    });
    expect(result.current.resumeOut).toBe('');
    expect(result.current.generating).toBe(false);
    expect(save).not.toHaveBeenCalled();
  });

  // The quality panel's Re-check is reachable ONLY on this surface's badge and
  // its result used to live in React state alone — reopening the job brought the
  // stale badge straight back. It must persist, and (Rust `merge_application`'s
  // `pick`: a blank incoming field keeps the stored one) it must carry the
  // report + the live texts and NOTHING else.
  describe('quality panel Re-check', () => {
    const FRESH_PAYLOAD = { ok: true, issues: [], metrics: EMPTY_METRICS };

    it('persists the merged wrapper against this job, with the texts intact', async () => {
      validateContent.mockResolvedValue(FRESH_PAYLOAD);
      const { result } = render();

      await act(async () => {
        await result.current.generate('my resume', 'both');
      });
      save.mockClear();

      await act(async () => {
        result.current.recheck?.();
      });

      // Validated the ACTIVE document's current text against the wizard résumé.
      expect(validateContent).toHaveBeenCalledWith(
        expect.objectContaining({
          generated: 'RESUME',
          source: 'my resume',
          jobAd: 'JD',
          docKind: 'resume',
        })
      );

      expect(save).toHaveBeenCalledTimes(1);
      const request = save.mock.calls[0]?.[0];
      expect(JSON.parse(String(request.qualityReport))).toEqual(
        expect.objectContaining({
          schemaVersion: 2,
          resume: { report: FRESH_PAYLOAD, sourceTextHash: hashText('RESUME') },
        })
      );
      // Texts carried live, never emptied…
      expect(request.resumeText).toBe('RESUME');
      expect(request.coverLetterText).toBe('COVER');
      // …routed to this job's aggregate…
      expect(request.jobUrl).toBe('https://acme.com/job/42');
      // …and every other field blank, so the merge keeps what is stored.
      expect(request.candidateName).toBe('');
      expect(request.jobAd).toBe('');
      expect(request.mode).toBe('');
      expect(request.topRequirements).toEqual([]);
    });

    it('clears staleness in the session too, so the badge recovers without a reload', async () => {
      validateContent.mockResolvedValue(FRESH_PAYLOAD);
      const { result } = render();

      await act(async () => {
        await result.current.generate('my resume', 'both');
      });
      await act(async () => {
        result.current.recheck?.();
      });

      expect(result.current.report?.resume).toEqual({
        report: FRESH_PAYLOAD,
        sourceTextHash: hashText('RESUME'),
      });
    });

    it('leaves the report and the record untouched when validation fails', async () => {
      validateContent.mockRejectedValue(new Error('boom'));
      const { result } = render();

      await act(async () => {
        await result.current.generate('my resume', 'both');
      });
      save.mockClear();

      await act(async () => {
        result.current.recheck?.();
      });

      expect(save).not.toHaveBeenCalled();
      expect(result.current.report).toBeNull();
    });
  });

  it('surfaces a rejected export as a notification instead of an unhandled rejection', async () => {
    vi.mocked(exportPDF).mockRejectedValueOnce(new Error('Export blocked: too many pages.'));
    const { result } = render();

    await act(async () => {
      await result.current.generate('my resume', 'resume');
    });

    // exportAs must resolve (not reject) even though the underlying export threw —
    // that's what turned this into an unhandled rejection with no toast.
    await act(async () => {
      await result.current.exportAs('pdf');
    });

    expect(mockNotify.error).toHaveBeenCalledWith({
      message: 'Export blocked: too many pages.',
    });
  });
});
