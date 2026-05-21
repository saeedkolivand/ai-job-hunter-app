import { CheckCircle2, XCircle } from 'lucide-react';

import { GlassCard } from '@ajh/ui';

import type { AnalysisResult } from '@/lib/resume-ai';

interface AnalysisStrengthsProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

export function AnalysisStrengths({ result, t }: AnalysisStrengthsProps) {
  return (
    <div className="grid gap-4 md:grid-cols-2">
      <GlassCard>
        <div className="mb-3 flex items-center gap-2">
          <CheckCircle2 size={13} className="text-emerald-400" />
          <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/40">
            {t('analyze.strengths')}
          </span>
          <span className="ml-auto text-[10px] text-foreground/30">
            {result.summary.strengths.length}
          </span>
        </div>
        <ul className="space-y-2">
          {result.summary.strengths.length === 0 && (
            <li className="text-xs text-foreground/30">{t('analyze.noStrengths')}</li>
          )}
          {result.summary.strengths.map((s, i) => (
            <li key={i} className="flex items-start gap-2 text-sm text-foreground/75">
              <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-400" />
              {s}
            </li>
          ))}
        </ul>
      </GlassCard>
      <GlassCard>
        <div className="mb-3 flex items-center gap-2">
          <XCircle size={13} className="text-amber-400" />
          <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/40">
            {t('analyze.weaknesses')}
          </span>
          <span className="ml-auto text-[10px] text-foreground/30">
            {result.summary.weaknesses.length}
          </span>
        </div>
        <ul className="space-y-2">
          {result.summary.weaknesses.length === 0 && (
            <li className="text-xs text-foreground/30">{t('analyze.noWeaknesses')}</li>
          )}
          {result.summary.weaknesses.map((w, i) => (
            <li key={i} className="flex items-start gap-2 text-sm text-foreground/75">
              <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-amber-400" />
              {w}
            </li>
          ))}
        </ul>
      </GlassCard>
    </div>
  );
}
