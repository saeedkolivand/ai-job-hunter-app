import { describe, expect, it } from 'vitest';

import {
  ANSWER_HEADROOM_TOKENS,
  assessModelContextFit,
  estimateTokensFromChars,
  MIN_USABLE_INPUT_TOKENS,
  STAGE_WORST_CASE_CHARS,
  worstCaseStage,
} from './context-fit';

describe('estimateTokensFromChars', () => {
  it('uses the shared ~4 chars/token ratio, not a second constant', () => {
    // The documented conversion: draft's ~32 000 chars ≈ 8 000 tokens.
    expect(estimateTokensFromChars(STAGE_WORST_CASE_CHARS.draft ?? 0)).toBe(8_000);
  });
});

describe('STAGE_WORST_CASE_CHARS', () => {
  it('matches the Rust draft worst case stated in the prompt module', () => {
    // candidate_resume (8k) + job_posting (8k) + resume_strategy artifact (16k).
    expect(STAGE_WORST_CASE_CHARS.draft).toBe(32_000);
  });

  it('charges analyze_job for the posting only', () => {
    expect(STAGE_WORST_CASE_CHARS.analyze_job).toBe(8_000);
  });
});

describe('worstCaseStage', () => {
  it('is COMPUTED, and it is not the draft stage', () => {
    const worst = worstCaseStage();

    // `strategy` carries two artifact-sized terms (evidence_map on top of
    // job_analysis) — naming `draft` in the UI copy would be false against
    // this module's own table.
    expect(worst.stage).toBe('strategy');
    expect(worst.chars).toBe(STAGE_WORST_CASE_CHARS.strategy);
    expect(worst.chars).toBeGreaterThan(STAGE_WORST_CASE_CHARS.draft ?? 0);
    expect(worst.tokens).toBe(estimateTokensFromChars(worst.chars));
  });
});

describe('assessModelContextFit', () => {
  it('says "unknown" when the window was never measured', () => {
    const fit = assessModelContextFit({ model: 'mystery' });

    // Absence of an /api/show answer is not evidence of a small window.
    expect(fit.verdict).toBe('unknown');
    expect(fit.overflowStages).toEqual([]);
    expect(fit.contextLength).toBeUndefined();
  });

  it('fits every stage on a large window', () => {
    const fit = assessModelContextFit({ model: 'big', contextLength: 131_072 });

    expect(fit.verdict).toBe('fits');
    expect(fit.overflowStages).toEqual([]);
    expect(fit.usableInputTokens).toBe(131_072 - ANSWER_HEADROOM_TOKENS);
  });

  it('is "tight" and NAMES the stages that would be cut', () => {
    // 12k window → ~10k usable: holds draft (8k) but not strategy (10k).
    const fit = assessModelContextFit({ model: 'mid', contextLength: 12_000 });

    expect(fit.verdict).toBe('tight');
    expect(fit.overflowStages).toContain('strategy');
    expect(fit.overflowStages).not.toContain('draft');
  });

  it('is "too-small" below the documented input floor', () => {
    const fit = assessModelContextFit({
      model: 'tiny',
      contextLength: MIN_USABLE_INPUT_TOKENS, // headroom pushes usable under the floor
    });

    expect(fit.verdict).toBe('too-small');
    expect(fit.overflowStages).toContain('draft');
  });

  it('counts the answer headroom against the window, not just the prompt', () => {
    // Exactly draft's input with nothing left for the answer → still overflows.
    const fit = assessModelContextFit({
      model: 'exact',
      contextLength: estimateTokensFromChars(STAGE_WORST_CASE_CHARS.draft ?? 0),
      stages: ['draft'],
    });

    expect(fit.overflowStages).toEqual(['draft']);
  });

  it('never reports a negative prompt budget', () => {
    // A window smaller than the answer headroom leaves ZERO for the prompt.
    const fit = assessModelContextFit({ model: 'tiny', contextLength: 512 });

    expect(fit.usableInputTokens).toBe(0);
    expect(fit.verdict).toBe('too-small');
  });

  it('scopes the verdict to the stages it was asked about', () => {
    const fit = assessModelContextFit({
      model: 'mid',
      contextLength: 16_384,
      stages: ['analyze_job', 'draft'],
    });

    expect(fit.verdict).toBe('fits');
  });
});
