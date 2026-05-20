import { describe, it, expect } from 'vitest';
import { MatchingEngine } from './engine';

const engine = new MatchingEngine();

describe('MatchingEngine.atsScore', () => {
  it('returns 1 when all requirements are covered', () => {
    const score = engine.atsScore(['typescript', 'react', 'node'], ['typescript', 'react', 'node']);
    expect(score).toBe(1);
  });

  it('returns 0 when nothing matches', () => {
    const score = engine.atsScore(['java', 'spring'], ['typescript', 'react']);
    expect(score).toBe(0);
  });

  it('returns partial score for partial overlap', () => {
    const score = engine.atsScore(['typescript', 'react'], ['typescript', 'react', 'graphql']);
    expect(score).toBeCloseTo(2 / 3);
  });

  it('returns 0 when requirements list is empty', () => {
    expect(engine.atsScore(['typescript'], [])).toBe(0);
  });

  it('is case-insensitive', () => {
    expect(engine.atsScore(['TypeScript'], ['typescript'])).toBe(1);
    expect(engine.atsScore(['REACT'], ['React'])).toBe(1);
  });

  it('normalizes whitespace', () => {
    expect(engine.atsScore(['node js'], ['node  js'])).toBe(1);
  });
});

describe('MatchingEngine.gaps', () => {
  it('returns missing requirements', () => {
    const gaps = engine.gaps(
      ['typescript', 'react'],
      ['typescript', 'react', 'graphql', 'postgres']
    );
    expect(gaps).toContain('graphql');
    expect(gaps).toContain('postgres');
    expect(gaps).not.toContain('typescript');
    expect(gaps).not.toContain('react');
  });

  it('returns empty array when all requirements are met', () => {
    expect(engine.gaps(['ts', 'react', 'node'], ['ts', 'react', 'node'])).toHaveLength(0);
  });

  it('returns all requirements when resume is empty', () => {
    const reqs = ['typescript', 'react'];
    expect(engine.gaps([], reqs)).toEqual(reqs);
  });
});

describe('MatchingEngine.combine', () => {
  it('uses 60% semantic weight by default', () => {
    const combined = engine.combine(0.5, 1.0);
    // ats * 0.4 + semantic * 0.6 = 0.2 + 0.6 = 0.8
    expect(combined).toBeCloseTo(0.8);
  });

  it('respects custom semantic weight', () => {
    const combined = engine.combine(1.0, 0.0, { semanticWeight: 0.5 });
    // ats * 0.5 + semantic * 0.5 = 0.5
    expect(combined).toBeCloseTo(0.5);
  });

  it('clamps output to 0–1', () => {
    expect(engine.combine(2, 2)).toBe(1);
    expect(engine.combine(-1, -1)).toBe(0);
  });
});

describe('MatchingEngine.evaluate', () => {
  it('returns a full score object', () => {
    const result = engine.evaluate('resume-1', 'job-1', {
      resumeText: 'Senior TypeScript developer with React and Node experience',
      resumeSkills: ['typescript', 'react', 'node'],
      jobText: 'We need TypeScript + React. GraphQL is a plus.',
      jobRequirements: ['typescript', 'react', 'graphql'],
      semanticSimilarity: 0.8,
    });

    expect(result.resumeId).toBe('resume-1');
    expect(result.jobId).toBe('job-1');
    expect(result.ats).toBeCloseTo(2 / 3);
    expect(result.semantic).toBe(0.8);
    expect(result.combined).toBeGreaterThan(0);
    expect(result.gaps).toContain('graphql');
    expect(result.gaps).not.toContain('typescript');
    expect(result.recommendations).toEqual([]);
  });
});
