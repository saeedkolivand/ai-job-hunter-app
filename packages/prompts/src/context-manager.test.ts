import { describe, expect, it } from 'vitest';

import {
  analyzeResumeSize,
  createCondensedResume,
  detectModelSize,
  detectSections,
  estimatePages,
  estimateTokens,
  getModelTier,
  getResumeStats,
  getStrategyForModel,
  LARGE_MODEL_STRATEGY,
  MEDIUM_MODEL_STRATEGY,
  SMALL_MODEL_STRATEGY,
  truncateResume,
} from './context-manager';

const RESUME = `John Doe
john@example.com | +1 555 0100

Professional Summary
Senior engineer with 10 years of experience building web platforms.

Work Experience
Acme Corp — Staff Engineer (2020 - Present)
Led the migration to microservices.
Improved latency by 40%.

Globex — Senior Engineer (2016 - 2020)
Built the billing system.

Education
BSc Computer Science, MIT

Skills
TypeScript, React, Node.js, AWS

Interests
Cycling, photography`;

describe('estimateTokens / estimatePages', () => {
  it('estimates tokens at ~4 chars each', () => {
    expect(estimateTokens('')).toBe(0);
    expect(estimateTokens('abcd')).toBe(1);
    expect(estimateTokens('abcde')).toBe(2);
  });

  it('estimates pages at ~3000 chars each', () => {
    expect(estimatePages('')).toBe(0);
    expect(estimatePages('a'.repeat(3000))).toBe(1);
    expect(estimatePages('a'.repeat(3001))).toBe(2);
  });
});

describe('detectSections', () => {
  it('detects standard resume sections including a header block', () => {
    const sections = detectSections(RESUME);
    const names = sections.map((s) => s.name);
    expect(names).toContain('Header');
    expect(names).toContain('Summary');
    expect(names).toContain('Experience');
    expect(names).toContain('Education');
    expect(names).toContain('Skills');
    expect(names).toContain('Interests');
  });

  it('assigns the highest priority to Experience', () => {
    const sections = detectSections(RESUME);
    const exp = sections.find((s) => s.name === 'Experience');
    expect(exp?.priority).toBe(10);
    expect(exp?.content).toContain('Acme Corp');
  });

  it('places pre-section content into a Header block', () => {
    const sections = detectSections('just a contact line, no headers');
    expect(sections.map((s) => s.name)).toEqual(['Header']);
  });
});

describe('detectModelSize / getModelTier', () => {
  it('classifies cloud/large models', () => {
    for (const name of ['gpt-4', 'claude-3', 'gemini-pro', 'mixtral', 'mistral-large']) {
      expect(detectModelSize(name)).toBe('large');
    }
  });

  it('classifies known small local models', () => {
    for (const name of ['llama3.2:1b', 'phi-3', 'gemma:2b', 'tinyllama', 'qwen2.5:3b']) {
      expect(detectModelSize(name)).toBe('small');
    }
  });

  it('parses the parameter size from the tag across styles', () => {
    expect(detectModelSize('llama3.1:8b')).toBe('medium');
    expect(detectModelSize('llama-3.2-1b')).toBe('small');
    expect(detectModelSize('qwen2.5:0.5b')).toBe('small');
    expect(detectModelSize('llama3:70b')).toBe('large');
    // hyphen + quantization / instruct suffixes don't swallow the size
    expect(detectModelSize('llama3.1:8b-instruct-q4_K_M')).toBe('medium');
    expect(detectModelSize('mistral:7b-instruct')).toBe('medium');
  });

  it('defaults unknown local models to small (safer prompt)', () => {
    expect(detectModelSize('some-random-model')).toBe('small');
    expect(detectModelSize('mistral')).toBe('small');
  });

  it('getModelTier mirrors detectModelSize', () => {
    expect(getModelTier('gpt-4')).toBe('large');
    expect(getModelTier('phi-3')).toBe('small');
    expect(getModelTier('mistral')).toBe('small');
  });
});

describe('getStrategyForModel', () => {
  it('returns the matching strategy per tier', () => {
    expect(getStrategyForModel('gpt-4')).toBe(LARGE_MODEL_STRATEGY);
    expect(getStrategyForModel('llama3.1:8b')).toBe(MEDIUM_MODEL_STRATEGY);
    expect(getStrategyForModel('phi-3')).toBe(SMALL_MODEL_STRATEGY);
  });
});

describe('truncateResume', () => {
  it('returns the resume unchanged when it is under the limit', () => {
    expect(truncateResume(RESUME, LARGE_MODEL_STRATEGY)).toBe(RESUME);
  });

  it('drops low-priority sections when over the limit', () => {
    const huge =
      'Professional Summary\n' +
      'x '.repeat(2000) +
      '\nWork Experience\nAcme — Engineer\n' +
      'y '.repeat(2000) +
      '\nInterests\nCycling';
    const result = truncateResume(huge, SMALL_MODEL_STRATEGY);
    expect(estimateTokens(result)).toBeLessThanOrEqual(SMALL_MODEL_STRATEGY.maxTokens + 50);
    expect(result).not.toContain('Cycling');
  });
});

describe('analyzeResumeSize', () => {
  it('reports single-pass for small resumes', () => {
    const result = analyzeResumeSize(RESUME);
    expect(result.strategy).toBe('single-pass');
    expect(result.totalTokens).toBeGreaterThan(0);
  });

  it('reports multi-pass when above the token budget', () => {
    const result = analyzeResumeSize('x'.repeat(40_000), 1000);
    expect(result.strategy).toBe('multi-pass');
  });
});

describe('createCondensedResume', () => {
  it('returns the original when single-pass', () => {
    expect(createCondensedResume(RESUME)).toBe(RESUME);
  });

  it('condenses very large resumes', () => {
    const huge =
      'Work Experience\n' +
      Array.from({ length: 50 }, (_, i) => `Company ${i} — Engineer\nDid work ${i}`).join('\n\n');
    const condensed = createCondensedResume(huge + '\n'.repeat(0) + 'z'.repeat(30_000));
    expect(condensed.length).toBeGreaterThan(0);
  });
});

describe('getResumeStats', () => {
  it('summarises a resume', () => {
    const stats = getResumeStats(RESUME);
    expect(stats.characters).toBe(RESUME.length);
    expect(stats.words).toBeGreaterThan(0);
    expect(stats.tokens).toBeGreaterThan(0);
    expect(stats.sections).toBeGreaterThan(0);
    expect(stats.needsTruncation).toBe(false);
    expect(stats.strategy).toBe('single-pass');
  });

  it('flags truncation for very large resumes', () => {
    const stats = getResumeStats('a'.repeat(30_000));
    expect(stats.needsTruncation).toBe(true);
    expect(stats.strategy).toBe('multi-pass');
  });
});
