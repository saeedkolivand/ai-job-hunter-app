import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { AnalysisResult } from '@/lib/resume-ai';

import { AnalysisResults } from './index';

const t = (k: string) => k;

function makeResult(overrides: Partial<AnalysisResult> = {}): AnalysisResult {
  return {
    detectedLanguages: { resume: 'en', jobAd: 'en', mismatch: false },
    scores: { ats: 80, jobMatch: 75, languageAlignment: 90, readability: 70, keywordCoverage: 65 },
    summary: {
      strengths: ['Strong React experience'],
      weaknesses: [],
      overallAssessment: 'Solid.',
    },
    missingKeywords: [],
    matchedSkills: ['React'],
    missingSkills: ['Docker'],
    recommendations: [{ priority: 'high', text: 'Add Terraform to skills', category: 'skill' }],
    sectionAnalysis: {
      summary: { score: 80, feedback: 'ok' },
      experience: { score: 75, feedback: 'ok' },
      skills: { score: 70, feedback: 'ok' },
      education: { score: 85, feedback: 'ok' },
      formatting: { score: 90, feedback: 'ok' },
    },
    rewrites: [],
    languageRecommendations: [],
    atsRisks: [],
    recruiterPerspective: 'Would shortlist.',
    finalVerdict: 'Good fit for this role',
    ...overrides,
  };
}

describe('AnalysisResults (#8/#9)', () => {
  it('leads with the verdict summary, always visible (not behind an accordion)', () => {
    render(<AnalysisResults result={makeResult()} t={t} />);
    expect(screen.getByText('Good fit for this role')).toBeInTheDocument();
  });

  it('opens the strengths & skills group by default, revealing its content', () => {
    render(<AnalysisResults result={makeResult()} t={t} />);
    expect(screen.getByText('analyze.groups.strengthsSkills')).toBeInTheDocument();
    // defaultOpen → the strength text is in the DOM without interaction.
    expect(screen.getByText('Strong React experience')).toBeInTheDocument();
  });

  it('keeps non-default groups collapsed (content not rendered until expanded)', () => {
    render(<AnalysisResults result={makeResult()} t={t} />);
    // The recommendations group header shows, but its body is collapsed.
    expect(screen.getByText('analyze.groups.recommendations')).toBeInTheDocument();
    expect(screen.queryByText('Add Terraform to skills')).toBeNull();
  });

  it('omits a group entirely when it has no content', () => {
    // No language mismatch and no language recommendations → no language group.
    render(<AnalysisResults result={makeResult()} t={t} />);
    expect(screen.queryByText('analyze.groups.language')).toBeNull();
  });

  it('shows the language group when there is a mismatch', () => {
    render(
      <AnalysisResults
        result={makeResult({
          detectedLanguages: { resume: 'en', jobAd: 'de', mismatch: true },
          languageRecommendations: ['Translate to German'],
        })}
        t={t}
      />
    );
    expect(screen.getByText('analyze.groups.language')).toBeInTheDocument();
  });
});
