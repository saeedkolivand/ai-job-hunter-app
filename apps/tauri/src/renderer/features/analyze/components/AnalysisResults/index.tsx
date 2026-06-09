import { Accordion } from '@ajh/ui';

import { AnalysisATSRisks } from '@/features/analyze/components/AnalysisATSRisks';
import { AnalysisLanguageMismatch } from '@/features/analyze/components/AnalysisLanguageMismatch';
import { AnalysisLanguageRecommendations } from '@/features/analyze/components/AnalysisLanguageRecommendations';
import { AnalysisMissingSkills } from '@/features/analyze/components/AnalysisMissingSkills';
import { AnalysisRecommendations } from '@/features/analyze/components/AnalysisRecommendations';
import { AnalysisRewrites } from '@/features/analyze/components/AnalysisRewrites';
import { AnalysisScores } from '@/features/analyze/components/AnalysisScores';
import { AnalysisSectionAnalysis } from '@/features/analyze/components/AnalysisSectionAnalysis';
import { AnalysisSkills } from '@/features/analyze/components/AnalysisSkills';
import { AnalysisStrengths } from '@/features/analyze/components/AnalysisStrengths';
import { AnalysisVerdict } from '@/features/analyze/components/AnalysisVerdict';
import type { AnalysisResult } from '@/lib/resume-ai';

interface AnalysisResultsProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

/**
 * Summary-first, categorized analysis view (#8/#9). The headline verdict + score
 * dimensions stay always-visible at the top (with a language-mismatch banner when
 * present); the detail — previously an 11-card text-bomb stacked vertically — is
 * grouped into collapsible sections so the page leads with the takeaway and the
 * rest is progressive disclosure. Each group renders only when it has content;
 * the first (strengths & skills) is open by default.
 */
export function AnalysisResults({ result, t }: AnalysisResultsProps) {
  const hasStrengthsSkills =
    result.summary.strengths.length > 0 ||
    result.summary.weaknesses.length > 0 ||
    result.matchedSkills.length > 0 ||
    result.missingSkills.length > 0;
  const hasRecommendations = result.recommendations.length > 0 || result.rewrites.length > 0;
  const hasAts = result.atsRisks.length > 0 || result.sectionAnalysis != null;
  const hasLanguage =
    result.detectedLanguages.mismatch || result.languageRecommendations.length > 0;

  return (
    <>
      {/* Summary first (#9) — banner (self-hides), headline verdict, then scores. */}
      <AnalysisLanguageMismatch result={result} t={t} />
      <AnalysisVerdict result={result} t={t} />
      <AnalysisScores result={result} t={t} />

      {/* Grouped detail (#8) — progressive disclosure, first group open. */}
      {hasStrengthsSkills && (
        <Accordion
          title={t('analyze.groups.strengthsSkills')}
          defaultOpen
          content={
            <div className="space-y-4">
              <AnalysisStrengths result={result} t={t} />
              <AnalysisSkills result={result} t={t} />
              <AnalysisMissingSkills result={result} />
            </div>
          }
        />
      )}
      {hasRecommendations && (
        <Accordion
          title={t('analyze.groups.recommendations')}
          content={
            <div className="space-y-4">
              <AnalysisRecommendations result={result} t={t} />
              <AnalysisRewrites result={result} />
            </div>
          }
        />
      )}
      {hasAts && (
        <Accordion
          title={t('analyze.groups.ats')}
          content={
            <div className="space-y-4">
              <AnalysisATSRisks result={result} t={t} />
              <AnalysisSectionAnalysis result={result} t={t} />
            </div>
          }
        />
      )}
      {hasLanguage && (
        <Accordion
          title={t('analyze.groups.language')}
          content={
            <div className="space-y-4">
              <AnalysisLanguageRecommendations result={result} t={t} />
            </div>
          }
        />
      )}
    </>
  );
}
