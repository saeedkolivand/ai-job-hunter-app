import { describe, expect, it } from 'vitest';

import { getModelGuidance } from './model-guidance';

describe('getModelGuidance (#6)', () => {
  it('flags a strong large cloud model for analysis', () => {
    expect(getModelGuidance('claude-opus-4-7', 'cloud')).toEqual({
      tier: 'large',
      light: false,
      task: 'strong',
      kind: 'cloud',
    });
  });

  it('calls out a hosted "mini" as a fast/light variant despite the large tier', () => {
    const g = getModelGuidance('gpt-4o-mini', 'cloud');
    expect(g.tier).toBe('large');
    expect(g.light).toBe(true);
    expect(g.task).toBe('light');
  });

  it('treats haiku / flash as light variants', () => {
    expect(getModelGuidance('claude-haiku-4-5-20251001', 'cloud').task).toBe('light');
    expect(getModelGuidance('gemini-2.5-flash', 'cloud').task).toBe('light');
  });

  it('maps local model sizes: small → light, medium → balanced', () => {
    expect(getModelGuidance('llama3.2:1b', 'local-server')).toMatchObject({
      tier: 'small',
      task: 'light',
      kind: 'local',
    });
    expect(getModelGuidance('llama3:8b', 'local-server')).toMatchObject({
      tier: 'medium',
      light: false,
      task: 'balanced',
      kind: 'local',
    });
  });

  it('maps provider kind to the right note (cli login, no key)', () => {
    expect(getModelGuidance('opus', 'cli-agent')).toMatchObject({ task: 'strong', kind: 'cli' });
  });
});
