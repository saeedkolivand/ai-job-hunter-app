import { CheckCircle2, AlertCircle } from 'lucide-react';
import { GlassCard } from '@/components/ui/GlassCard';
import type { AnalysisResult } from '@/lib/resume-ai';

interface AnalysisSkillsProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

export function AnalysisSkills({ result, t }: AnalysisSkillsProps) {
  return (
    <div className="grid gap-4 md:grid-cols-2">
      {result.matchedSkills.length > 0 && (
        <GlassCard>
          <div className="mb-3 flex items-center gap-2">
            <CheckCircle2 size={12} className="text-emerald-400" />
            <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/40">
              {t('analyze.matchedSkills')}
            </span>
          </div>
          <div className="flex flex-wrap gap-1.5">
            {result.matchedSkills.map((s) => (
              <span
                key={s}
                className="rounded-full border border-emerald-400/20 bg-emerald-400/5 px-2.5 py-1 text-[11px] text-emerald-300/90"
              >
                {s}
              </span>
            ))}
          </div>
        </GlassCard>
      )}
      {result.missingKeywords.length > 0 && (
        <GlassCard>
          <div className="mb-3 flex items-center gap-2">
            <AlertCircle size={12} className="text-amber-400" />
            <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/40">
              {t('analyze.gaps')}
            </span>
          </div>
          <div className="flex flex-wrap gap-1.5">
            {result.missingKeywords.map((s) => (
              <span
                key={s}
                className="rounded-full border border-amber-400/20 bg-amber-400/5 px-2.5 py-1 text-[11px] text-amber-300/90"
              >
                {s}
              </span>
            ))}
          </div>
        </GlassCard>
      )}
    </div>
  );
}
