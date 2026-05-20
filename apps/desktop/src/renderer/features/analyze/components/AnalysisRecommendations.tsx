import { Sparkles } from 'lucide-react';
import { cn } from '@/lib/cn';
import { GlassCard } from '@/components/ui/GlassCard';
import type { AnalysisResult } from '@/lib/resume-ai';

interface AnalysisRecommendationsProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

export function AnalysisRecommendations({ result, t }: AnalysisRecommendationsProps) {
  if (result.recommendations.length === 0) return null;

  const categoryBadge = {
    keyword: t('analyze.categories.keyword'),
    skill: t('analyze.categories.skill'),
    format: t('analyze.categories.format'),
    language: t('analyze.categories.language'),
    experience: t('analyze.categories.experience'),
  };

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <Sparkles size={13} className="text-brand-soft" />
        <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/40">
          {t('analyze.recommendations')}
        </span>
        <span className="ml-auto text-[10px] text-foreground/30">
          {result.recommendations.length}
        </span>
      </div>
      <div className="space-y-2.5">
        {result.recommendations.map((r, i) => {
          const priorityColor =
            r.priority === 'high'
              ? 'bg-red-400'
              : r.priority === 'medium'
                ? 'bg-amber-400'
                : 'bg-blue-400';
          return (
            <div key={i} className="flex items-start gap-3 rounded-lg bg-white/[0.02] px-3 py-2.5">
              <span className={cn('mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full', priorityColor)} />
              <div className="flex-1 text-sm text-foreground/75">{r.text}</div>
              <span className="shrink-0 rounded-md border border-white/[0.06] bg-white/[0.03] px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-wider text-foreground/30">
                {categoryBadge[r.category]}
              </span>
            </div>
          );
        })}
      </div>
    </GlassCard>
  );
}
