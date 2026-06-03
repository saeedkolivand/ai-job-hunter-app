import { ArrowRight } from 'lucide-react';

import { GlassCard } from '@ajh/ui';

import type { AnalysisResult } from '@/lib/resume-ai';

interface AnalysisRewritesProps {
  result: AnalysisResult;
}

export function AnalysisRewrites({ result }: AnalysisRewritesProps) {
  if (result.rewrites.length === 0) return null;

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <ArrowRight size={13} className="text-brand-soft" />
        <span className="text-xs font-semibold uppercase tracking-[0.16em] text-foreground/55">
          Suggested Rewrites
        </span>
      </div>
      <div className="space-y-4">
        {result.rewrites.map((rw, i) => (
          <div key={i} className="rounded-xl border border-white/[0.06] overflow-hidden">
            <div className="border-b border-white/[0.06] px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-foreground/55">
              {rw.section}
            </div>
            <div className="grid md:grid-cols-2 divide-y md:divide-y-0 md:divide-x divide-white/[0.06]">
              <div className="px-3 py-2.5">
                <div className="mb-1 text-[9px] font-semibold uppercase tracking-wider text-foreground/55">
                  Before
                </div>
                <div className="text-xs text-foreground/45 leading-relaxed">{rw.original}</div>
              </div>
              <div className="px-3 py-2.5">
                <div className="mb-1 text-[9px] font-semibold uppercase tracking-wider text-emerald-400/50">
                  After
                </div>
                <div className="text-xs text-foreground/75 leading-relaxed">{rw.improved}</div>
              </div>
            </div>
            {rw.reason && (
              <div className="border-t border-white/[0.06] px-3 py-1.5 text-[10px] text-foreground/30">
                💡 {rw.reason}
              </div>
            )}
          </div>
        ))}
      </div>
    </GlassCard>
  );
}
