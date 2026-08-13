import { describe, expect, it } from 'vitest';

import { assessThinkingRisk, HEAVY_RATIO, MEASURED_MIN_CALLS } from './thinking-risk';

const measuredRow = (over: Partial<Record<string, unknown>> = {}) => ({
  provider: 'openai',
  model: 'o4-mini',
  calls: MEASURED_MIN_CALLS,
  thinkingTokens: 10_000,
  outputTokens: 1_000,
  ...over,
});

describe('assessThinkingRisk — measured signal', () => {
  it('reports a heavy MEASURED ratio with its sample size', () => {
    const risk = assessThinkingRisk({
      model: 'o4-mini',
      measured: [measuredRow()],
    });

    expect(risk.level).toBe('measured-heavy');
    expect(risk.ratio).toBe(10);
    expect(risk.calls).toBe(MEASURED_MIN_CALLS);
  });

  it('ignores a ratio from too few calls (noise, per the spend read model)', () => {
    const risk = assessThinkingRisk({
      model: 'o4-mini',
      measured: [measuredRow({ calls: MEASURED_MIN_CALLS - 1 })],
    });

    expect(risk.level).toBe('unmeasured');
  });

  it('reports a MEASURED-and-fine model as measured, not as unmeasured', () => {
    const risk = assessThinkingRisk({
      model: 'o4-mini',
      measured: [measuredRow({ thinkingTokens: (HEAVY_RATIO - 1) * 1_000 })],
    });

    // `unmeasured` copy says "local models never report a split" — claiming
    // that about a model the provider DID measure is simply false.
    expect(risk.level).toBe('measured-light');
    expect(risk.ratio).toBe(HEAVY_RATIO - 1);
  });

  it('treats thinking with ZERO answer tokens as the heavy case', () => {
    const risk = assessThinkingRisk({
      model: 'o4-mini',
      measured: [measuredRow({ outputTokens: 0, thinkingTokens: 50_000 })],
    });

    expect(risk.level).toBe('measured-heavy');
    // No denominator exists, so no ratio may be invented for the copy.
    expect(risk.ratio).toBeUndefined();
  });

  it('stays unmeasured when the row is all zeroes', () => {
    const risk = assessThinkingRisk({
      model: 'o4-mini',
      measured: [measuredRow({ outputTokens: 0, thinkingTokens: 0 })],
    });

    expect(risk.level).toBe('unmeasured');
  });
});

describe('assessThinkingRisk — name signal (the local-only case)', () => {
  it('flags a thinking-heavy family at high effort as a known-bad combo', () => {
    // The 22.9:1 gpt-oss:20b case: Ollama never reports the split, so the name
    // is the ONLY signal that exists for it.
    const risk = assessThinkingRisk({ model: 'gpt-oss:20b', effort: 'high', measured: [] });

    expect(risk.level).toBe('known-bad-combo');
    expect(risk.effort).toBe('high');
  });

  it('downgrades the same model at a lower effort', () => {
    const risk = assessThinkingRisk({ model: 'gpt-oss:20b', effort: 'low', measured: [] });

    expect(risk.level).toBe('reasoning-heavy');
  });

  it('matches the other reasoning families by name', () => {
    for (const model of ['deepseek-r1:14b', 'qwq:32b', 'magistral-small', 'qwen3-thinking']) {
      expect(assessThinkingRisk({ model, effort: 'high' }).level).toBe('known-bad-combo');
    }
  });
});

describe('assessThinkingRisk — absence of data', () => {
  it('reports an unmeasured model as UNMEASURED, not as safe', () => {
    const risk = assessThinkingRisk({ model: 'llama3.2:1b', effort: 'high', measured: [] });

    // The level name is the assertion: nothing here may read as "no risk".
    expect(risk.level).toBe('unmeasured');
    expect(risk.ratio).toBeUndefined();
  });

  it('does not borrow another model’s measurement', () => {
    const risk = assessThinkingRisk({
      model: 'llama3.2:1b',
      measured: [measuredRow({ model: 'o4-mini' })],
    });

    expect(risk.level).toBe('unmeasured');
  });

  it('does not borrow the same NAME’s measurement from another provider', () => {
    // The same model id can sit behind two providers; only one of them was
    // measured, and a local run of it is still unmeasured.
    const risk = assessThinkingRisk({
      model: 'gpt-oss:20b',
      provider: 'ollama',
      measured: [measuredRow({ provider: 'openai-compatible', model: 'gpt-oss:20b' })],
      effort: 'low',
    });

    expect(risk.level).toBe('reasoning-heavy');
  });

  it('uses the measurement when the provider matches', () => {
    const risk = assessThinkingRisk({
      model: 'gpt-oss:20b',
      provider: 'openai-compatible',
      measured: [measuredRow({ provider: 'openai-compatible', model: 'gpt-oss:20b' })],
    });

    expect(risk.level).toBe('measured-heavy');
  });
});
