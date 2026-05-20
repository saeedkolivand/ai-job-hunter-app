import { Globe } from 'lucide-react';
import { GlassCard } from '@/components/ui/GlassCard';
import type { AnalysisResult } from '@/lib/resume-ai';

interface AnalysisLanguageRecommendationsProps {
  result: AnalysisResult;
  t: (key: string) => string;
}

export function AnalysisLanguageRecommendations({
  result,
  t,
}: AnalysisLanguageRecommendationsProps) {
  if (result.languageRecommendations.length === 0) return null;

  return (
    <GlassCard>
      <div className="mb-3 flex items-center gap-2">
        <Globe size={13} className="text-blue-400" />
        <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/40">
          {t('analyze.languageRecommendations')}
        </span>
      </div>
      <ul className="space-y-2">
        {result.languageRecommendations.map((r, i) => (
          <li key={i} className="flex items-start gap-2 text-sm text-foreground/65">
            <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-blue-400" />
            {r}
          </li>
        ))}
      </ul>
    </GlassCard>
  );
}
