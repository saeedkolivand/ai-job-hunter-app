import { cn } from '@ajh/ui';

export type ScoreTier = 'High' | 'Medium' | 'Low';

/** Map a 0–100 match score to a Low / Medium / High tier (#52/#50). */
export function scoreTier(value: number): { label: ScoreTier; cls: string } {
  if (value >= 75)
    return { label: 'High', cls: 'border-emerald-400/25 bg-emerald-400/10 text-emerald-300' };
  if (value >= 50)
    return { label: 'Medium', cls: 'border-amber-400/25 bg-amber-400/10 text-amber-300' };
  return { label: 'Low', cls: 'border-red-400/25 bg-red-400/10 text-red-300' };
}

/** Low/Medium/High pill for a match score. */
export function MatchBand({ value, large }: { value: number; large?: boolean }) {
  const band = scoreTier(value);
  return (
    <span
      className={cn(
        'inline-flex items-center rounded-full border font-semibold uppercase tracking-wider',
        band.cls,
        large ? 'px-2.5 py-1 text-xs' : 'px-2 py-0.5 text-[10px]'
      )}
    >
      {band.label}
    </span>
  );
}
