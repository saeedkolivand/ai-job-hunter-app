import { describe, expect, it } from 'vitest';

import { isPayingPipelineStage, isPipelineStage, PIPELINE_STAGES } from '@ajh/shared';

import {
  OVERRIDABLE_PIPELINE_STAGES,
  providerNeedsModel,
  resolveActiveProblem,
  resolveStageRouting,
} from './stage-routing';

const allConfigured = () => true;

describe('OVERRIDABLE_PIPELINE_STAGES', () => {
  it('is the generated vocabulary minus the stages that make no provider call', () => {
    expect(OVERRIDABLE_PIPELINE_STAGES).toEqual(PIPELINE_STAGES.filter(isPayingPipelineStage));
    // The two the backend refuses an override for.
    expect(OVERRIDABLE_PIPELINE_STAGES).not.toContain('assemble');
    expect(OVERRIDABLE_PIPELINE_STAGES).not.toContain('validate');
  });

  it('keeps every stage that DOES call a provider, in pipeline order', () => {
    expect([...OVERRIDABLE_PIPELINE_STAGES]).toEqual([
      'analyze_job',
      'match_evidence',
      'strategy',
      'draft',
      'sections',
      'repair',
      'llm_judge',
    ]);
  });

  it('offers only names the closed vocabulary actually contains', () => {
    for (const stage of OVERRIDABLE_PIPELINE_STAGES) {
      expect(isPipelineStage(stage)).toBe(true);
    }
  });
});

describe('resolveStageRouting', () => {
  it('reports the ACTIVE provider for a stage with no override', () => {
    const [row] = resolveStageRouting({
      overrides: {},
      active: { provider: 'ollama', model: 'qwen3:8b' },
      isConfigured: allConfigured,
      stages: ['draft'],
    });

    expect(row).toMatchObject({ provider: 'ollama', model: 'qwen3:8b', problem: undefined });
    // Absence of an override is the state the UI must render as "default".
    expect(row?.override).toBeUndefined();
  });

  it('reports the override’s own provider + model when one is set', () => {
    const [row] = resolveStageRouting({
      overrides: { draft: { provider: 'openai', model: 'gpt-5.1' } },
      active: { provider: 'ollama', model: 'qwen3:8b' },
      isConfigured: allConfigured,
      stages: ['draft'],
    });

    expect(row).toMatchObject({ provider: 'openai', model: 'gpt-5.1' });
    expect(row?.override).toEqual({ provider: 'openai', model: 'gpt-5.1' });
  });

  it('flags an override pointing at an UNCONFIGURED provider', () => {
    const [row] = resolveStageRouting({
      overrides: { llm_judge: { provider: 'anthropic', model: 'claude-sonnet-4-6' } },
      active: { provider: 'ollama', model: 'qwen3:8b' },
      isConfigured: (p) => p === 'ollama',
      stages: ['llm_judge'],
    });

    // The backend refuses the run rather than falling back, so this has to be
    // visible before the run — not after it fails.
    expect(row?.problem).toBe('unconfigured');
  });

  it('flags an override on a configured provider with no model', () => {
    const [row] = resolveStageRouting({
      overrides: { strategy: { provider: 'openai', model: '' } },
      active: { provider: 'ollama', model: 'qwen3:8b' },
      isConfigured: allConfigured,
      stages: ['strategy'],
    });

    expect(row?.problem).toBe('no-model');
  });

  it('does NOT repeat the ACTIVE provider’s problem on every row', () => {
    const rows = resolveStageRouting({
      overrides: {},
      active: { provider: 'openai' },
      isConfigured: allConfigured,
    });

    // One global problem is reported once, by `resolveActiveProblem` — not as
    // seven identical warnings offering advice ("put this step back on the
    // default") that a row already on the default cannot follow.
    expect(rows.every((r) => r.problem === undefined)).toBe(true);
  });

  it('accepts a CLI agent with no model — it runs its own default', () => {
    const [row] = resolveStageRouting({
      overrides: { strategy: { provider: 'claude-code', model: '' } },
      active: { provider: 'ollama', model: 'qwen3:8b' },
      isConfigured: allConfigured,
      stages: ['strategy'],
    });

    expect(row?.problem).toBeUndefined();
    expect(providerNeedsModel('claude-code')).toBe(false);
    expect(providerNeedsModel('openai')).toBe(true);
  });

  it('returns one row per overridable stage', () => {
    const rows = resolveStageRouting({
      overrides: {},
      active: {},
      isConfigured: () => false,
    });

    expect(rows).toHaveLength(OVERRIDABLE_PIPELINE_STAGES.length);
  });
});

describe('resolveActiveProblem', () => {
  it('reports an unconfigured active provider once', () => {
    expect(
      resolveActiveProblem({
        active: { provider: 'anthropic', model: 'claude-sonnet-4-6' },
        isConfigured: () => false,
      })
    ).toBe('unconfigured');
  });

  it('reports a configured active provider with no model', () => {
    expect(
      resolveActiveProblem({ active: { provider: 'openai' }, isConfigured: allConfigured })
    ).toBe('no-model');
  });

  it('reports nothing when the active provider can actually run', () => {
    expect(
      resolveActiveProblem({
        active: { provider: 'ollama', model: 'qwen3:8b' },
        isConfigured: allConfigured,
      })
    ).toBeUndefined();
  });

  it('treats a missing provider as unconfigured', () => {
    expect(resolveActiveProblem({ active: {}, isConfigured: allConfigured })).toBe('unconfigured');
  });
});
