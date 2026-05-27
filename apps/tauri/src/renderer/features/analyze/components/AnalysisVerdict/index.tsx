import { Sparkles } from 'lucide-react';

import { cn, GlassCard } from '@ajh/ui';

import { type AnalysisResult, verdictGradient } from '@/lib/resume-ai';

interface AnalysisVerdictProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

export function AnalysisVerdict({ result, t }: AnalysisVerdictProps) {
  const gradient = verdictGradient(result.scores.jobMatch);

  return (
    <GlassCard>
      <div className="flex items-start gap-4">
        <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-brand/15">
          <Sparkles size={20} className="text-brand-soft" />
        </div>
        <div className="flex-1">
          <div className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/35">
            {t('analyze.verdict')}
          </div>
          <div
            className={cn(
              'mt-1 text-base font-semibold bg-gradient-to-r bg-clip-text text-transparent',
              gradient
            )}
          >
            {result.finalVerdict}
          </div>
          {result.recruiterPerspective && (
            <div className="mt-2 text-sm text-foreground/55 leading-relaxed">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-foreground/30">
                {t('analyze.recruiterView')}{' '}
              </span>
              {result.recruiterPerspective}
            </div>
          )}
        </div>
      </div>
    </GlassCard>
  );
}
