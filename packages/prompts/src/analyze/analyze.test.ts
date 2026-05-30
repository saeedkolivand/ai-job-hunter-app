import { describe, expect, it } from 'vitest';

import {
  type AnalysisResult,
  buildAnalysisPrompt,
  buildSystemPrompt,
  validateAndRepair,
} from './index';

describe('buildSystemPrompt', () => {
  it('returns the full three-perspective prompt for large models', () => {
    const prompt = buildSystemPrompt('large');
    expect(prompt).toContain('PERSPECTIVE 1');
    expect(prompt).toContain('PERSPECTIVE 2');
    expect(prompt).toContain('PERSPECTIVE 3');
  });

  it('returns a compact prompt for small models', () => {
    const prompt = buildSystemPrompt('small');
    expect(prompt).toContain('resume reviewer');
    expect(prompt.length).toBeLessThan(buildSystemPrompt('large').length);
  });
});

describe('buildAnalysisPrompt', () => {
  const resume = 'Professional Summary\nEngineer.\nSkills\nReact';
  const jobAd = 'We need a React engineer.';

  it('emits the stepwise prompt for large models', () => {
    const prompt = buildAnalysisPrompt(resume, jobAd, {}, 'large');
    expect(prompt).toContain('ANALYSIS STEPS');
    expect(prompt).toContain('STEP 6');
  });

  it('emits a compact prompt for small models', () => {
    const prompt = buildAnalysisPrompt(resume, jobAd, {}, 'small');
    expect(prompt).toContain('Think step by step');
    expect(prompt).not.toContain('STEP 10');
  });

  it('includes pre-detected language hints when provided', () => {
    const prompt = buildAnalysisPrompt(resume, jobAd, {
      resumeLanguage: 'en',
      jobAdLanguage: 'de',
    });
    expect(prompt).toContain('PRE-DETECTED LANGUAGES');
    expect(prompt).toContain('Language mismatch: true');
  });

  it('honours tone and target locale notes', () => {
    const prompt = buildAnalysisPrompt(resume, jobAd, {
      outputTone: 'formal',
      targetLocale: 'de',
    });
    expect(prompt).toContain('formal tone');
    expect(prompt).toContain('in de');
  });

  it('resolves the tier from the model name when no tier is passed', () => {
    const large = buildAnalysisPrompt(resume, jobAd, { modelName: 'gpt-4' });
    const small = buildAnalysisPrompt(resume, jobAd, { modelName: 'phi-3' });
    expect(large.length).toBeGreaterThan(small.length);
  });
});

describe('validateAndRepair', () => {
  const minimal = JSON.stringify({
    detectedLanguages: { resume: 'en', jobAd: 'en', mismatch: false },
    scores: { ats: 80, jobMatch: 70, languageAlignment: 100, readability: 65, keywordCoverage: 60 },
    summary: { strengths: ['x'], weaknesses: ['y'], overallAssessment: 'ok' },
    missingKeywords: ['Kubernetes'],
    matchedSkills: ['React'],
    missingSkills: [],
    recommendations: [{ priority: 'high', text: 'Add Kubernetes', category: 'keyword' }],
    sectionAnalysis: {
      summary: { score: 80, feedback: 'good' },
      experience: { score: 70, feedback: 'ok' },
      skills: { score: 60, feedback: 'ok' },
      education: { score: 90, feedback: 'good' },
      formatting: { score: 85, feedback: 'good' },
    },
    rewrites: [{ section: 'exp', original: 'a', improved: 'b', reason: 'clearer' }],
    languageRecommendations: [],
    atsRisks: [{ severity: 'high', issue: 'tables', fix: 'remove' }],
    recruiterPerspective: 'solid',
    finalVerdict: 'apply',
  });

  it('parses a complete result', () => {
    const result = validateAndRepair(minimal) as AnalysisResult;
    expect(result).not.toBeNull();
    expect(result.scores.ats).toBe(80);
    expect(result.recommendations[0]?.priority).toBe('high');
    expect(result.rewrites).toHaveLength(1);
  });

  it('extracts JSON wrapped in a code fence', () => {
    const fenced = '```json\n' + minimal + '\n```';
    expect(validateAndRepair(fenced)).not.toBeNull();
  });

  it('repairs trailing commas', () => {
    const broken = '{"scores":{"ats":50,},"recruiterPerspective":"x",}';
    const result = validateAndRepair(broken);
    expect(result).not.toBeNull();
    expect(result?.scores.ats).toBe(50);
  });

  it('clamps out-of-range scores and coerces invalid enums', () => {
    const result = validateAndRepair(
      JSON.stringify({
        scores: { ats: 150, jobMatch: -10 },
        recommendations: [{ priority: 'urgent', text: 'do it', category: 'bogus' }],
      })
    ) as AnalysisResult;
    expect(result.scores.ats).toBe(100);
    expect(result.scores.jobMatch).toBe(0);
    expect(result.recommendations[0]?.priority).toBe('medium');
    expect(result.recommendations[0]?.category).toBe('skill');
  });

  it('drops recommendations and rewrites with empty required fields', () => {
    const result = validateAndRepair(
      JSON.stringify({
        recommendations: [{ priority: 'low', text: '', category: 'skill' }],
        rewrites: [{ section: 's', original: 'o', improved: '', reason: 'r' }],
      })
    ) as AnalysisResult;
    expect(result.recommendations).toHaveLength(0);
    expect(result.rewrites).toHaveLength(0);
  });

  it('returns null when no JSON object is present', () => {
    expect(validateAndRepair('totally not json')).toBeNull();
  });

  it('defaults missing sub-objects gracefully', () => {
    const result = validateAndRepair('{}') as AnalysisResult;
    expect(result.detectedLanguages.resume).toBe('unknown');
    expect(result.scores.ats).toBe(50); // clamp() default for NaN
    expect(result.sectionAnalysis.summary.feedback).toBe('No feedback provided.');
  });
});
