import { Globe } from 'lucide-react';
import type { AnalysisResult } from '@/lib/resume-ai';

interface AnalysisLanguageMismatchProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

export function AnalysisLanguageMismatch({ result, t }: AnalysisLanguageMismatchProps) {
  if (!result.detectedLanguages.mismatch) return null;

  return (
    <div className="flex items-start gap-3 rounded-xl border border-amber-400/20 bg-amber-400/[0.06] px-4 py-3">
      <Globe size={16} className="mt-0.5 shrink-0 text-amber-400" />
      <div>
        <div className="text-sm font-medium text-amber-300/90">{t('analyze.languageMismatch')}</div>
        <div className="mt-0.5 text-xs text-amber-200/60">
          {(t as (key: string, opts: Record<string, unknown>) => string)(
            'analyze.languageMismatchBody',
            { resume: result.detectedLanguages.resume, jobAd: result.detectedLanguages.jobAd }
          )}
        </div>
      </div>
    </div>
  );
}
