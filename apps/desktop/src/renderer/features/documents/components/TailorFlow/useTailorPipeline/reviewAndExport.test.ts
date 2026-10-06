/**
 * useTailorPipeline — section-fix / fabrication review, openClaimsTotal, export market, resolveTargetLanguage precedence.
 * Mocks + render helpers live in `harness.ts` (see its header).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import {
  detail,
  ENGLISH_JOB_AD,
  exportDOCX,
  exportPDF,
  GERMAN_JOB_AD,
  record,
  regenerateMutate,
  render,
  resetHarness,
  resolveFabricationMutate,
  resolveTargetLanguage,
  sessionBus,
} from './harness';

beforeEach(resetHarness);

describe('useTailorPipeline — the section-fix / fabrication-review bundle', () => {
  it('is undefined until a run detail exists', () => {
    const { result } = render();
    expect(result.current.pipelineReview).toBeUndefined();
  });

  it('wires onFixSection to regenerateSection keyed on the run id', () => {
    sessionBus.detail = detail({ runId: 'run-7' });
    const { result } = render();
    expect(result.current.pipelineReview).toBeDefined();

    result.current.pipelineReview?.onFixSection?.('skills', 'be more specific');
    expect(regenerateMutate).toHaveBeenCalledWith({
      runId: 'run-7',
      sectionKey: 'skills',
      note: 'be more specific',
    });
  });

  it('wires onResolveFabrication to resolveFabrication keyed on the run id', () => {
    sessionBus.detail = detail({ runId: 'run-7' });
    const { result } = render();

    result.current.pipelineReview?.onResolveFabrication?.('code#0', 'keep');
    expect(resolveFabricationMutate).toHaveBeenCalledWith({
      runId: 'run-7',
      issueKey: 'code#0',
      decision: 'keep',
    });
  });
});

describe('useTailorPipeline — openClaimsTotal counts BOTH slots (H5)', () => {
  const minimalReport = {
    ok: true,
    issues: [],
    metrics: {
      keywordCoverage: null,
      topRequirementHits: null,
      duplicateRatio: 0,
      rolesSource: 0,
      rolesOutput: 0,
    },
  };

  it('sums unresolved fabrications from resume AND coverLetter, not just the active tab', () => {
    sessionBus.detail = detail({
      resumeText: 'Résumé text mentions FOO-EVIDENCE right here.',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 0,
        resume: {
          report: minimalReport,
          sourceTextHash: 0,
          fabrications: [{ issueKey: 'a#0', code: 'a', evidence: 'FOO-EVIDENCE' }],
        },
        coverLetter: {
          report: minimalReport,
          sourceTextHash: 0,
          fabrications: [{ issueKey: 'b#0', code: 'b', evidence: 'BAR-EVIDENCE' }],
        },
      },
    });
    const generation = record({
      id: 'gen-1',
      coverLetterText: 'Cover letter mentions BAR-EVIDENCE right here.',
    });

    const { result } = render({ latestGeneration: generation });

    // `activeOut` defaults to 'resume' — the OLD, buggy single-slot count
    // would report 1 here (only the resume's own fabrication).
    expect(result.current.activeOut).toBe('resume');
    expect(result.current.openClaimsTotal).toBe(2);
  });

  it('is 0 when neither slot has an unresolved fabrication', () => {
    sessionBus.detail = detail({
      resumeText: 'Clean résumé text.',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 0,
        resume: { report: minimalReport, sourceTextHash: 0 },
      },
    });
    const { result } = render();
    expect(result.current.openClaimsTotal).toBe(0);
  });
});

// The bug: `exportAs` used to pass the literal `undefined` in the `locale`
// position of both `exportPDF`/`exportDOCX`, which the Rust exporter resolves
// to market "intl" — silently dropping market-specific conventions (e.g. DIN
// 5008 for a German letter). `resolveMarket` is now computed once from the
// hook's own `targetLanguage` (detected from `jobDesc`) and threaded through.
// Real (unmocked) `detectLanguage` fixtures below are lifted verbatim from
// `packages/shared/src/language-detection.test.ts` — known-reliable inputs.
describe('useTailorPipeline — export market (DIN 5008 / locale-drop regression)', () => {
  /** Exports a résumé in `format` for `jobDesc` and returns the locale argument it was sent. */
  async function exportLocale(format: 'pdf' | 'docx', jobDesc: string) {
    sessionBus.detail = detail({ resumeText: 'RESUME TEXT' });
    const { result } = render({ jobDesc });
    await act(async () => {
      await result.current.exportAs(format);
    });
    return vi.mocked(format === 'pdf' ? exportPDF : exportDOCX).mock.calls.at(-1)?.[6];
  }

  it.each([
    [
      'sends the German market (not undefined, not "intl") for a German job ad — PDF',
      'pdf',
      GERMAN_JOB_AD,
      'de',
    ],
    ['sends the German market for a German job ad — DOCX', 'docx', GERMAN_JOB_AD, 'de'],
    [
      'sends the English/international market for an English job ad — PDF',
      'pdf',
      ENGLISH_JOB_AD,
      'intl',
    ],
  ] as const)('%s', async (_name, format, jobDesc, market) => {
    expect(await exportLocale(format, jobDesc)).toBe(market);
  });

  it('never leaves the locale argument undefined, regardless of language', async () => {
    expect(await exportLocale('docx', ENGLISH_JOB_AD)).not.toBeUndefined();
  });

  // The live preview (GenerationOutput → PdfPreview) reads this SAME value off
  // the hook's return, not the export call args — without it exposed here the
  // preview silently renders under market "intl" while the export renders under
  // "de" (a German posting shows an English salutation on screen but a German
  // one in the downloaded file). Asserting the exposed value directly, not just
  // the export call, is what would fail if a future edit dropped it from the
  // return object.
  // `jobLocation` is the found job's free-text location (e.g. "New York, NY,
  // US") — previously never read, so an ENGLISH posting always fell through
  // to `LANGUAGE_TO_MARKET.en === 'intl'` (A4) even for a US applicant.
  it.each([
    [
      'exposes the resolved market on the hook return (not just the export call)',
      { jobDesc: GERMAN_JOB_AD },
      'de',
    ],
    [
      'a US-located English posting resolves market "us" (US Letter), not "intl"',
      { jobDesc: ENGLISH_JOB_AD, jobLocation: 'New York, NY, US' },
      'us',
    ],
    [
      'an unlocated English posting still falls back to "intl"',
      { jobDesc: ENGLISH_JOB_AD, jobLocation: undefined },
      'intl',
    ],
  ])('%s', (_name, overrides, market) => {
    sessionBus.detail = detail({ resumeText: 'RESUME TEXT' });
    const { result } = render(overrides);

    expect(result.current.market).toBe(market);
  });
});

// `resolveTargetLanguage` — the pure precedence chain `useTailorPipeline`'s
// `targetLanguage` memo wraps. Tested directly (no hook, no session mock) per
// the plan's "extract into a pure exported helper" note; the hook-level
// tests cover the WIRING (the memo's output actually reaching
// `session.start`/`meta`), not the precedence logic itself.
describe('resolveTargetLanguage — precedence chain (Defect A/B fix)', () => {
  const DE = { language: 'de', confident: true };

  it.each([
    [
      'prefers the persisted targetLanguage — the field the staged pipeline actually writes — over everything else',
      record({ targetLanguage: 'de', jobAdLanguage: 'en' }),
      '',
      DE,
    ],
    // The English-lock regression test (Defect B): a résumé written in English
    // must NEVER pin the target the moment the pipeline hasn't confidently
    // written one yet — only the job ad's own language may. Mutation: read
    // `latestGeneration.resumeLanguage` back into the chain (tier 1 or 2) →
    // this goes red (`'en'` instead of `'de'`).
    [
      'ignores the source résumé language entirely, even when present on the record',
      record({ resumeLanguage: 'English', targetLanguage: '', jobAdLanguage: '' }),
      GERMAN_JOB_AD,
      DE,
    ],
    // The SAME persisted field carries two shapes: `extractMetadata` (the
    // AIGeneratePage flow) writes a display NAME like "German", every other
    // writer an ISO code, and `save_application` merges both into one record.
    // Preferring "German" verbatim is WORSE than the bug this chain fixes —
    // Rust truncates it to "ge", which matches no language arm, so the
    // language checks go dark for that document.
    [
      'normalizes a persisted language NAME to its ISO code before preferring it',
      record({ targetLanguage: 'German', jobAdLanguage: '' }),
      '',
      DE,
    ],
    // Each tier is validated INDEPENDENTLY: an invalid-but-present targetLanguage
    // must not short-circuit a perfectly good jobAdLanguage one rung down.
    [
      'falls through an invalid targetLanguage to a valid jobAdLanguage',
      record({ targetLanguage: 'not-a-language', jobAdLanguage: 'de' }),
      '',
      DE,
    ],
    [
      'falls back to jobAdLanguage when targetLanguage is empty',
      record({ targetLanguage: '', jobAdLanguage: 'de' }),
      '',
      DE,
    ],
    [
      'detects the language from the job ad when there is no previous generation (first run)',
      undefined,
      GERMAN_JOB_AD,
      DE,
    ],
    // The negative case the owner explicitly asked to be pinned: when nothing
    // is confident, the chain still returns a usable code (generation must
    // proceed) — but flags it `confident: false` so the caller keeps it off
    // the wire. Mutation: return `confident: true` unconditionally → red.
    [
      'marks the last-resort English fallback as NOT confident, unlike a real detection',
      undefined,
      '',
      { language: 'en', confident: false },
    ],
  ])('%s', (_name, generation, jobDesc, expected) => {
    expect(resolveTargetLanguage(generation, jobDesc)).toEqual(expected);
  });
});
