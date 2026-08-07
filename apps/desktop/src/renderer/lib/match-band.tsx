import { useTranslation } from '@ajh/translations';
import { cn, Tag, type TagStatusColor } from '@ajh/ui';

type ScoreTier = 'High' | 'Medium' | 'Low';

/** Map a 0–100 score to a tier + colour.
 *
 *  variant='combined' (default) — combined semantic+ATS score, 75/50 cut points.
 *  variant='coverage'           — raw ATS keyword-coverage score; coverage clusters
 *                                 lower than combined, so cut points are relaxed.
 *                                 // ponytail: heuristic starting values, not calibrated
 */
export function scoreTier(
  value: number,
  variant: 'combined' | 'coverage' = 'combined'
): { key: ScoreTier; color: TagStatusColor } {
  if (variant === 'coverage') {
    if (value >= 55) return { key: 'High', color: 'success' };
    if (value >= 30) return { key: 'Medium', color: 'warning' };
    return { key: 'Low', color: 'error' };
  }
  if (value >= 75) return { key: 'High', color: 'success' };
  if (value >= 50) return { key: 'Medium', color: 'warning' };
  return { key: 'Low', color: 'error' };
}

/** i18n key for the plain-language explanation of a tier.
 *
 *  Keyed on the VARIANT too, because the two say genuinely different things: a
 *  `combined` High blends meaning + keywords, a `coverage` High is keyword
 *  overlap alone with no AI scoring behind it. One shared sentence would have
 *  to be vague enough to be true of both, which is how a tooltip stops being
 *  worth reading.
 */
export function matchBandDescriptionKey(
  tier: ScoreTier,
  variant: 'combined' | 'coverage' = 'combined'
): string {
  return `jobs.matchBand.desc.${variant}.${tier}`;
}

/** Low/Medium/High match-score Tag — localized label.
 *
 *  subtle=true: Medium/Low render muted-neutral; High stays bright — the tier
 *  itself is trustworthy, only the LOWER tiers are de-emphasized. Used in
 *  compact list rows; detail pane always passes subtle=false (default).
 *
 *  muted=true: EVERY tier (including High) renders muted-neutral — a stronger,
 *  distinct de-emphasis for when the SCORE ITSELF is approximate/provisional
 *  (not the tier), so a "confident High" never reads as more certain than it
 *  actually is. Deliberately a separate prop from `subtle`, not an extension
 *  of it — `subtle`'s High-stays-bright contract is pinned by its own tests
 *  and other callers may rely on it.
 *
 *  describe=true (default): the bare word "High" means nothing on its own, so
 *  the band carries a plain-language explanation as a native `title` plus an
 *  sr-only suffix. Deliberately NOT a `HoverPopover` like TrustBadge uses: this
 *  band renders inside a `<Button>` on the Autopilot card (a focusable popover
 *  trigger there is invalid button-in-button HTML) and inside
 *  aria-activedescendant listbox rows (where it would add a tab stop per row).
 *  `title` is hover-only and focusable-free, so it is safe in both.
 *  Pass describe={false} where the caller already surfaces richer copy in its
 *  own popover — see RowMatchScore — so the two don't both fire on hover.
 */
export function MatchBand({
  value,
  large,
  variant = 'combined',
  subtle = false,
  muted = false,
  describe = true,
}: {
  value: number;
  large?: boolean;
  variant?: 'combined' | 'coverage';
  subtle?: boolean;
  muted?: boolean;
  describe?: boolean;
}) {
  const { t } = useTranslation();
  const band = scoreTier(value, variant);
  // `muted` mutes unconditionally (all tiers); `subtle` mutes only Medium/Low.
  const isMutedStyle = muted || (subtle && band.key !== 'High');
  const description = describe ? t(matchBandDescriptionKey(band.key, variant)) : undefined;

  const tag = (
    <Tag
      color={isMutedStyle ? undefined : band.color}
      className={cn(
        'rounded-full font-semibold uppercase tracking-wider',
        large ? 'px-2.5 py-1 text-xs' : 'px-2 py-0.5 text-[10px]',
        isMutedStyle && 'bg-muted text-foreground/70'
      )}
    >
      {t(`jobs.matchBand.${band.key}`)}
    </Tag>
  );

  if (!description) return tag;

  return (
    <span className="inline-flex items-center" title={description}>
      {tag}
      <span className="sr-only">: {description}</span>
    </span>
  );
}
