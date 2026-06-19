import { cn, Tag, type TagStatusColor } from '@ajh/ui';

export type ScoreTier = 'High' | 'Medium' | 'Low';

/** Map a 0–100 match score to a Low / Medium / High tier (#52/#50), with the
 *  matching {@link Tag} status colour (theme-safe in light + dark). */
function scoreTier(value: number): { label: ScoreTier; color: TagStatusColor } {
  if (value >= 75) return { label: 'High', color: 'success' };
  if (value >= 50) return { label: 'Medium', color: 'warning' };
  return { label: 'Low', color: 'error' };
}

/** Low/Medium/High match-score Tag. */
export function MatchBand({ value, large }: { value: number; large?: boolean }) {
  const band = scoreTier(value);
  return (
    <Tag
      color={band.color}
      className={cn(
        'rounded-full font-semibold uppercase tracking-wider',
        large ? 'px-2.5 py-1 text-xs' : 'px-2 py-0.5 text-[10px]'
      )}
    >
      {band.label}
    </Tag>
  );
}
