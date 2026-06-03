import { motion } from 'motion/react';

import { cn, transition } from '@ajh/ui';

import { type AnalysisResult, scoreLabel, verdictGradient } from '@/lib/resume-ai';

interface AnalysisScoresProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

export function AnalysisScores({ result, t }: AnalysisScoresProps) {
  const gradient = verdictGradient(result.scores.jobMatch);

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
        const { label: sl, color } = scoreLabel(val);
        return (
          <div
            key={key}
            className="relative overflow-hidden rounded-xl border border-foreground/[0.08] bg-foreground/[0.03] px-4 py-4"
          >
            <div className="text-[10px] font-medium uppercase tracking-[0.16em] text-foreground/35">
              {label}
            </div>
            <div className="mt-1.5 text-3xl font-bold tabular-nums text-foreground">{val}</div>
            <div className={cn('mt-0.5 text-[10px] font-medium', color)}>{sl}</div>
            <div className="mt-2 h-1 overflow-hidden rounded-full bg-foreground/10">
              <motion.div
                initial={{ width: 0 }}
                animate={{ width: `${val}%` }}
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
