import { Gauge } from 'lucide-react';

import type { MatchScore } from '@ajh/shared';
import { Button, GlassCard } from '@ajh/ui';

import { useDocuments, useMatchResume } from '@/services';

interface RawDoc {
  _id: string;
  isDefault?: boolean;
}

/** Resolve the default saved resume's real id (`_id`), or the first saved one. */
function useDefaultResumeId(): string | null {
  const { data = [] } = useDocuments();
  const docs = data as unknown as RawDoc[];
  const def = docs.find((d) => d.isDefault) ?? docs[0];
  return def?._id ?? null;
}

function ScoreBar({ label, value }: { label: string; value: number }) {
  return (
    <div>
      <div className="mb-1 flex items-center justify-between text-[11px]">
        <span className="text-foreground/60">{label}</span>
        <span className="text-foreground/80">{Math.round(value)}%</span>
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-white/5">
        <div className="h-full rounded-full bg-brand" style={{ width: `${Math.round(value)}%` }} />
      </div>
    </div>
  );
}

export function MatchScoreCard({ jobId }: { jobId: string }) {
  const resumeId = useDefaultResumeId();
  const match = useMatchResume();
  const result: (MatchScore & { error?: string }) | undefined = match.data;

  return (
    <GlassCard tone="violet" className="mb-4">
      <div className="mb-3 flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs font-medium text-foreground/90">
          <Gauge size={14} className="text-brand" />
          Resume match
        </div>
        <Button
          size="sm"
          variant="ghost"
          disabled={!resumeId || match.isPending}
          loading={match.isPending}
          onClick={() => resumeId && match.mutate({ resumeId, jobId })}
        >
          {result ? 'Re-score' : 'Score'}
        </Button>
      </div>

      {!resumeId && (
        <div className="text-[11px] text-foreground/45">
          Save a resume first to score it against this job.
        </div>
      )}

      {match.isError && (
        <div className="text-[11px] text-amber-300/90">
          {match.error instanceof Error ? match.error.message : 'Match failed.'}
        </div>
      )}

      {result?.error && <div className="text-[11px] text-amber-300/90">{result.error}</div>}

      {result && !result.error && (
        <div className="space-y-3">
          <div className="flex items-baseline gap-2">
            <span className="text-2xl font-semibold text-foreground">
              {Math.round(result.combined)}%
            </span>
            <span className="text-[11px] text-foreground/45">overall match</span>
          </div>
          <ScoreBar label="Semantic" value={result.semantic} />
          <ScoreBar label="Keyword coverage (ATS)" value={result.ats} />

          {result.gaps.length > 0 && (
            <div>
              <div className="mb-1.5 text-[10px] uppercase tracking-[0.18em] text-foreground/40">
                Missing keywords
              </div>
              <div className="flex flex-wrap gap-1.5">
                {result.gaps.map((g) => (
                  <span
                    key={g}
                    className="rounded-md bg-white/5 px-2 py-0.5 text-[11px] text-foreground/70"
                  >
                    {g}
                  </span>
                ))}
              </div>
            </div>
          )}

          {result.recommendations.map((r) => (
            <div key={r} className="text-[11px] text-foreground/55">
              {r}
            </div>
          ))}
        </div>
      )}
    </GlassCard>
  );
}
