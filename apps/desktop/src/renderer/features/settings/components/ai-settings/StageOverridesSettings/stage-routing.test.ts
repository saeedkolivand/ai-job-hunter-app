import { describe, expect, it } from 'vitest';

import { isPipelineStage, PIPELINE_STAGES, PIPELINE_STAGES_FREE } from '@ajh/shared';

import {
  OVERRIDABLE_PIPELINE_STAGES,
  providerNeedsModel,
  resolveStageRouting,
} from './stage-routing';

const allConfigured = () => true;

describe('OVERRIDABLE_PIPELINE_STAGES', () => {
  it('is the generated vocabulary minus the stages that make no provider call', () => {
    expect(OVERRIDABLE_PIPELINE_STAGES).toEqual(
      PIPELINE_STAGES.filter((s) => !(PIPELINE_STAGES_FREE as readonly string[]).includes(s))
    );
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

  it('flags a configured provider with no model', () => {
    const [row] = resolveStageRouting({
      overrides: {},
      active: { provider: 'openai' },
      isConfigured: allConfigured,
      stages: ['strategy'],
    });

    expect(row?.problem).toBe('no-model');
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

  it('flags every stage when nothing is configured at all', () => {
    const rows = resolveStageRouting({
      overrides: {},
      active: {},
      isConfigured: () => false,
    });

    expect(rows).toHaveLength(OVERRIDABLE_PIPELINE_STAGES.length);
    expect(rows.every((r) => r.problem === 'unconfigured')).toBe(true);
  });
});
