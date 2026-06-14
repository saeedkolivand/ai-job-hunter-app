import { motion } from 'motion/react';

import { cn, transition } from '@ajh/ui';

import { type AnalysisResult, scoreLabel, verdictGradient } from '@/lib/resume-ai';

interface AnalysisScoresProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

export function AnalysisScores({ result, t }: AnalysisScoresProps) {
  // jobMatch may be null ("not scored") — fall back to the lowest-tier gradient
  // for the bars rather than feeding null into verdictGradient (number-typed).
  const gradient = verdictGradient(result.scores.jobMatch ?? 0);

  const scoreConfigs = [
    { key: 'ats' as const, label: t('analyze.resultScores.ats') },
    { key: 'jobMatch' as const, label: t('analyze.resultScores.jobMatch') },
    { key: 'keywordCoverage' as const, label: t('analyze.resultScores.keywordCoverage') },
    { key: 'readability' as const, label: t('analyze.resultScores.readability') },
    { key: 'languageAlignment' as const, label: t('analyze.resultScores.languageAlignment') },
  ] as const;

  return (
    <div className="grid grid-cols-5 gap-3">
      {scoreConfigs.map(({ key, label }) => {
        const val = result.scores[key];
        // null = "not scored": show an honest placeholder instead of a number,
        // suppress the (fabricated) score-tier label, and leave the bar empty.
        const scored = val !== null;
        const { label: sl, color } = scored
          ? scoreLabel(val)
          : { label: t('analyze.notScored'), color: 'text-foreground/40' };
        return (
          <div
            key={key}
            className="surface-card relative overflow-hidden rounded-xl px-4 py-4 shadow-sm"
          >
            <div className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/55">
              {label}
            </div>
            <div
              className={cn(
                'mt-1.5 font-bold tabular-nums',
                scored ? 'text-3xl text-foreground' : 'text-base text-foreground/40'
              )}
            >
              {scored ? val : t('analyze.notScored')}
            </div>
            {scored && <div className={cn('mt-0.5 text-[10px] font-medium', color)}>{sl}</div>}
            <div className="mt-2 h-1 overflow-hidden rounded-full bg-foreground/10">
              <motion.div
                initial={{ width: 0 }}
                animate={{ width: `${scored ? val : 0}%` }}
                transition={transition.dataBar}
                className={cn('h-full rounded-full bg-gradient-to-r', gradient)}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
