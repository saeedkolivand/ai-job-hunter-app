import { useTranslation } from '@ajh/translations';

interface Props {
  /** Open fabrication claims awaiting a verdict. */
  openClaims: number;
  /** Cold redisplay — no interactive fix/resolve UI is wired, so the hint says to reopen the run. */
  cold: boolean;
}

/** Amber status block under the run banner when a run finished but needs the user's review. */
export function NeedsReviewNotice({ openClaims, cold }: Props) {
  const { t } = useTranslation();
  return (
    <div role="status" className="rounded-lg border border-amber-400/20 bg-amber-400/5 px-3 py-2.5">
      {openClaims > 0 ? (
        <>
          <p className="text-[11px] font-medium text-amber-400">
            {t('autopilot.apply.wizard.results.needsReviewTitle', { count: openClaims })}
          </p>
          <p className="mt-1 text-[10px] leading-relaxed text-foreground/50">
            {t(
              cold
                ? 'autopilot.apply.wizard.results.needsReviewHintCold'
                : 'autopilot.apply.wizard.results.needsReviewHint'
            )}
          </p>
        </>
      ) : (
        // The report can flag `needsReview` on a critical with NO
        // fabrication entry (Rust `slot_has_unresolvable_critical`) —
        // "0 claims need your verdict" then points at an empty list.
        // Say what's actually true and point at the quality badge
        // instead of a per-claim count that doesn't exist.
        <>
          <p className="text-[11px] font-medium text-amber-400">
            {t('autopilot.apply.wizard.results.needsReviewTitleEmpty')}
          </p>
          <p className="mt-1 text-[10px] leading-relaxed text-foreground/50">
            {t('autopilot.apply.wizard.results.needsReviewHintEmpty')}
          </p>
        </>
      )}
    </div>
  );
}
