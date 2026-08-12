import { describe, expect, it } from 'vitest';

import { EXTRACTION_STAGES, suggestExtractionModel } from './suggest-stage-models';

const base = {
  activeProvider: 'ollama',
  activeModel: 'qwen3:32b',
  installedModels: ['qwen3:32b', 'llama3.2:1b', 'nomic-embed-text'],
  overrides: {},
};

describe('suggestExtractionModel', () => {
  it('suggests a smaller installed model for the three extraction stages', () => {
    expect(suggestExtractionModel(base)).toEqual({
      model: 'llama3.2:1b',
      stages: [...EXTRACTION_STAGES],
    });
  });

  it('never suggests an embedding model', () => {
    const suggestion = suggestExtractionModel({
      ...base,
      installedModels: ['qwen3:32b', 'nomic-embed-text'],
    });

    // `nomic-embed-text` parses as no size (→ "small") but cannot answer a
    // chat turn at all, so "smaller" must not be the whole test.
    expect(suggestion).toBeNull();
  });

  it('is silent when nothing installed is smaller than the active model', () => {
    expect(
      suggestExtractionModel({
        ...base,
        activeModel: 'llama3.2:1b',
        installedModels: ['llama3.2:1b', 'qwen3:32b'],
      })
    ).toBeNull();
  });

  it('is silent for a cloud provider — a local model is not the fix there', () => {
    expect(
      suggestExtractionModel({ ...base, activeProvider: 'openai', activeModel: 'gpt-5.1' })
    ).toBeNull();
  });

  it('is silent when no model is active yet', () => {
    expect(suggestExtractionModel({ ...base, activeModel: undefined })).toBeNull();
  });

  it('offers only the stages the user has not already pinned', () => {
    const suggestion = suggestExtractionModel({
      ...base,
      overrides: { analyze_job: { provider: 'ollama', model: 'llama3.2:1b' } },
    });

    expect(suggestion?.stages).toEqual(['match_evidence', 'strategy']);
  });

  it('is silent once every extraction stage is pinned', () => {
    expect(
      suggestExtractionModel({
        ...base,
        overrides: Object.fromEntries(
          EXTRACTION_STAGES.map((s) => [s, { provider: 'ollama', model: 'llama3.2:1b' }])
        ),
      })
    ).toBeNull();
  });

  it('picks the smallest tier, not merely the first smaller one', () => {
    const suggestion = suggestExtractionModel({
      ...base,
      // medium (8b) listed before small (1b): order must not beat size.
      installedModels: ['qwen3:32b', 'qwen3:8b', 'llama3.2:1b'],
    });

    expect(suggestion?.model).toBe('llama3.2:1b');
  });
});
