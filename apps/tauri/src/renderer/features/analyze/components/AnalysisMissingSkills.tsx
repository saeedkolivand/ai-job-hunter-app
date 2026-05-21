import { XCircle } from 'lucide-react';

import { GlassCard } from '@ajh/ui';

import type { AnalysisResult } from '@/lib/resume-ai';

interface AnalysisMissingSkillsProps {
  result: AnalysisResult;
}

export function AnalysisMissingSkills({ result }: AnalysisMissingSkillsProps) {
  if (result.missingSkills.length === 0) return null;

  return (
    <GlassCard>
      <div className="mb-3 flex items-center gap-2">
        <XCircle size={12} className="text-red-400" />
        <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/40">
          Missing Skills (Broader Gaps)
        </span>
      </div>
      <div className="flex flex-wrap gap-1.5">
        {result.missingSkills.map((s) => (
          <span
            key={s}
            className="rounded-full border border-red-400/15 bg-red-400/[0.04] px-2.5 py-1 text-[11px] text-red-300/70"
          >
            {s}
          </span>
        ))}
      </div>
    </GlassCard>
  );
}
