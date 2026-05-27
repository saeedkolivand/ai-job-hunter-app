import { ArrowRight, ShieldAlert } from 'lucide-react';

import { GlassCard } from '@ajh/ui';

import { cn } from '@ajh/ui';
import type { AnalysisResult } from '@/lib/resume-ai';

interface AnalysisATSRisksProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

export function AnalysisATSRisks({ result, t }: AnalysisATSRisksProps) {
  if (result.atsRisks.length === 0) return null;

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <ShieldAlert size={13} className="text-red-400" />
        <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/40">
          {t('analyze.atsRisks')}
        </span>
      </div>
      <div className="space-y-2">
        {result.atsRisks.map((risk, i) => {
          const colors =
            risk.severity === 'high'
              ? {
                  border: 'border-red-400/20',
                  bg: 'bg-red-400/[0.04]',
                  dot: 'bg-red-400',
                  text: 'text-red-300/80',
                }
              : risk.severity === 'medium'
                ? {
                    border: 'border-amber-400/20',
                    bg: 'bg-amber-400/[0.04]',
                    dot: 'bg-amber-400',
                    text: 'text-amber-300/80',
                  }
                : {
                    border: 'border-blue-400/20',
                    bg: 'bg-blue-400/[0.04]',
                    dot: 'bg-blue-400',
                    text: 'text-blue-300/80',
                  };
          return (
            <div key={i} className={cn('rounded-lg border px-3 py-2.5', colors.border, colors.bg)}>
              <div className="flex items-center gap-2 text-xs font-medium text-foreground/80">
                <span className={cn('h-1.5 w-1.5 rounded-full', colors.dot)} />
                {risk.issue}
              </div>
              {risk.fix && (
                <div className={cn('mt-1 flex items-center gap-1 text-xs', colors.text)}>
                  <ArrowRight size={10} /> {risk.fix}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </GlassCard>
  );
}
