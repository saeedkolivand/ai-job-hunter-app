import { motion } from 'motion/react';

import { cn } from '@ajh/ui';
import { transition } from '@ajh/ui';
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
            className="relative overflow-hidden rounded-xl border border-white/[0.07] px-4 py-4"
            style={{
              background: 'linear-gradient(135deg, rgba(255,255,255,0.03), rgba(255,255,255,0.01))',
            }}
          >
            <div className="text-[10px] font-medium uppercase tracking-[0.16em] text-foreground/35">
              {label}
            </div>
            <div className="mt-1.5 text-3xl font-semibold tabular-nums text-foreground/90">
              {val}
            </div>
            <div className={cn('mt-0.5 text-[10px] font-medium', color)}>{sl}</div>
            <div className="mt-2 h-1 overflow-hidden rounded-full bg-white/[0.06]">
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
