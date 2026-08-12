import { describe, expect, it } from 'vitest';

import {
  EXTRACTION_STAGES,
  isChatCapable,
  parseParameterSizeB,
  suggestExtractionModel,
} from './suggest-stage-models';

/** `/api/show` shapes, as Ollama actually reports them. */
const INSPECTIONS = {
  'qwen3:32b': { parameterSize: '32.8B', family: 'qwen3', contextLength: 40_960 },
  'qwen3:8b': { parameterSize: '8.2B', family: 'qwen3', contextLength: 40_960 },
  'llama3.2:1b': { parameterSize: '1.2B', family: 'llama', contextLength: 131_072 },
  // The three executed counter-cases from review.
  'llama3.3:latest': { parameterSize: '70.6B', family: 'llama', contextLength: 131_072 },
  qwq: { parameterSize: '32.8B', family: 'qwen2', contextLength: 40_960 },
  'all-minilm': { parameterSize: '22.7M', family: 'bert', contextLength: 512 },
};

const base = {
  activeProvider: 'ollama',
  activeModel: 'qwen3:32b',
  installedModels: ['qwen3:32b', 'llama3.2:1b'],
  inspections: INSPECTIONS,
  overrides: {},
};

describe('parseParameterSizeB', () => {
  it('parses the units Ollama reports', () => {
    expect(parseParameterSizeB('8.2B')).toBeCloseTo(8.2);
    expect(parseParameterSizeB('22.7M')).toBeCloseTo(0.0227);
  });

  it('returns null for anything it cannot parse — never a default', () => {
    for (const value of [undefined, '', 'latest', 'unknown', 'big']) {
      expect(parseParameterSizeB(value)).toBeNull();
    }
  });
});

describe('isChatCapable', () => {
  it('rejects an embedding model by its reported family, not its name', () => {
    // `all-minilm` contains no "embed" substring — the name check alone let it
    // through and it was recommended for three JSON chat stages.
    expect(isChatCapable('all-minilm', INSPECTIONS['all-minilm'])).toBe(false);
  });

  it('accepts a chat family', () => {
    expect(isChatCapable('qwen3:8b', INSPECTIONS['qwen3:8b'])).toBe(true);
  });
});

describe('suggestExtractionModel', () => {
  it('suggests a measurably smaller installed model for the reading stages', () => {
    expect(suggestExtractionModel(base)).toEqual({
      model: 'llama3.2:1b',
      stages: [...EXTRACTION_STAGES],
      candidateB: 1.2,
      activeB: 32.8,
    });
  });

  it('REGRESSION: never calls a 70B model "smaller" than an 8B one', () => {
    // The name heuristic parsed `llama3.3:latest` as small (no size in the tag).
    expect(
      suggestExtractionModel({
        ...base,
        activeModel: 'qwen3:8b',
        installedModels: ['qwen3:8b', 'llama3.3:latest'],
      })
    ).toBeNull();
  });

  it('REGRESSION: never suggests an embedding model', () => {
    expect(
      suggestExtractionModel({ ...base, installedModels: ['qwen3:32b', 'all-minilm'] })
    ).toBeNull();
  });

  it('REGRESSION: never suggests a model the risk step flags as reasoning-heavy', () => {
    // `qwq` is the same size as the active model, so the size gate alone stops
    // it — the suggestion and the warning can no longer contradict each other.
    expect(suggestExtractionModel({ ...base, installedModels: ['qwen3:32b', 'qwq'] })).toBeNull();
  });

  it('is silent when the ACTIVE model has no measured size', () => {
    expect(
      suggestExtractionModel({
        ...base,
        activeModel: 'mystery:latest',
        installedModels: ['mystery:latest', 'llama3.2:1b'],
      })
    ).toBeNull();
  });

  it('is silent when the candidate has no measured size', () => {
    expect(
      suggestExtractionModel({
        ...base,
        installedModels: ['qwen3:32b', 'unmeasured:latest'],
        inspections: { ...INSPECTIONS, 'unmeasured:latest': null },
      })
    ).toBeNull();
  });

  it('ignores a candidate that is not meaningfully smaller', () => {
    expect(
      suggestExtractionModel({
        ...base,
        activeModel: 'qwen3:8b',
        installedModels: ['qwen3:8b', 'qwen3-7b'],
        inspections: { ...INSPECTIONS, 'qwen3-7b': { parameterSize: '7.6B', family: 'qwen3' } },
      })
    ).toBeNull();
  });

  it('picks the LARGEST model that clears the bar, not the tiniest thing installed', () => {
    const suggestion = suggestExtractionModel({
      ...base,
      installedModels: ['qwen3:32b', 'llama3.2:1b', 'qwen3:8b'],
    });

    // 8.2B is 4× smaller than 32.8B and still the most capable option.
    expect(suggestion?.model).toBe('qwen3:8b');
  });

  it('is silent for a cloud provider — a local model is not the fix there', () => {
    expect(
      suggestExtractionModel({ ...base, activeProvider: 'openai', activeModel: 'gpt-5.1' })
    ).toBeNull();
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
});
