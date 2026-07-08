import { describe, expect, it } from 'vitest';

import { buildSystemPrompt } from '../analyze';
import { resolveProfile } from './index';

describe('resolveProfile — provider → prompt depth', () => {
  it('ollama small → brief, compact schema, no rewrites/structured output', () => {
    const r = resolveProfile({ kind: 'ollama', sizeHint: 'small' });
    expect(r.depth).toBe('brief');
    expect(r.schema).toBe('compact');
    expect(r.includeRewrites).toBe(false);
    expect(r.structuredOutput).toBe(false);
  });

  it('ollama medium → brief (splits the old medium == large)', () => {
    expect(resolveProfile({ kind: 'ollama', sizeHint: 'medium' }).depth).toBe('brief');
  });

  it('ollama large → full', () => {
    expect(resolveProfile({ kind: 'ollama', sizeHint: 'large' }).depth).toBe('full');
  });

  it('cloud → full, rich schema, structured output', () => {
    const r = resolveProfile({ kind: 'cloud' });
    expect(r.depth).toBe('full');
    expect(r.schema).toBe('full');
    expect(r.includeRewrites).toBe(true);
    expect(r.structuredOutput).toBe(true);
  });

  it('cli → task brief', () => {
    expect(resolveProfile({ kind: 'cli' }).depth).toBe('task');
  });

  it('derives the ollama sub-tier from the model tag', () => {
    expect(resolveProfile({ kind: 'ollama', model: 'llama3.2:1b' }).depth).toBe('brief');
    expect(resolveProfile({ kind: 'ollama', model: 'llama3:70b' }).depth).toBe('full');
  });

  it('legacy tier strings remain backward compatible', () => {
    expect(resolveProfile('large').depth).toBe('full');
    expect(resolveProfile('medium').depth).toBe('brief');
    expect(resolveProfile('small').depth).toBe('brief');
  });

  it('respects an explicit supportsStructuredOutput flag', () => {
    expect(
      resolveProfile({ kind: 'cloud', supportsStructuredOutput: false }).structuredOutput
    ).toBe(false);
    expect(
      resolveProfile({ kind: 'ollama', supportsStructuredOutput: true }).structuredOutput
    ).toBe(true);
  });
});

describe('buildSystemPrompt — provider-aware depth markers', () => {
  it('compact for ollama small, full for cloud, task brief for cli', () => {
    const small = buildSystemPrompt({ kind: 'ollama', sizeHint: 'small' });
    const cloud = buildSystemPrompt({ kind: 'cloud' });
    const cli = buildSystemPrompt({ kind: 'cli' });

    expect(small.length).toBeLessThan(cloud.length);
    expect(cloud).toContain('three simultaneous perspectives');
    expect(cli).toContain('TASK');
    expect(cli).toContain('ACCEPTANCE CHECKS');
  });
});
