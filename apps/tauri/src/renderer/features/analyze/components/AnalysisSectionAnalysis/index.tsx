import { FileText } from 'lucide-react';

import { cn, GlassCard } from '@ajh/ui';

import { type AnalysisResult, scoreLabel } from '@/lib/resume-ai';

interface AnalysisSectionAnalysisProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

export function AnalysisSectionAnalysis({ result, t }: AnalysisSectionAnalysisProps) {
  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <FileText size={13} className="text-brand-soft" />
        <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          {t('analyze.sectionAnalysis')}
        </span>
      </div>
      <div className="space-y-3">
        {(
          Object.entries(result.sectionAnalysis) as [
            string,
            { score: number | null; feedback: string },
          ][]
        ).map(([key, sec]) => {
          // null score = "not scored": show the placeholder, not a fabricated number.
          const color = sec.score !== null ? scoreLabel(sec.score).color : 'text-foreground/40';
          return (
            <div key={key} className="grid grid-cols-[80px_40px_1fr] items-center gap-3">
              <div className="text-xs capitalize text-foreground/55">{key}</div>
              <div className={cn('text-sm font-semibold tabular-nums', color)}>
                {sec.score !== null ? sec.score : '—'}
              </div>
              <div className="text-xs text-foreground/45 leading-snug">{sec.feedback}</div>
            </div>
          );
        })}
      </div>
    </GlassCard>
  );
}
