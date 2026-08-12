import { Check, FileQuestion, ShieldAlert, Trash2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { errorDetail } from '@/lib/error-class';
import { type Fabrication, presentFabrication, unresolvedCount } from '@/lib/generate';

export interface FabricationReviewProps {
  entries: Fabrication[];
  /** The document as it stands NOW — decides whether an entry's evidence can
   *  still be found (see the `orphaned` presentation). */
  documentText: string;
  /** Record one verdict. Omit to render the list read-only (an older run). */
  onResolve?: (issueKey: string, decision: 'remove' | 'keep') => void;
  /**
   * APPLY a Remove verdict to the document: delete the flagged line(s) and put
   * the result through the host's ordinary edit/save path. Called BEFORE
   * `onResolve`, so the recorded decision and the text agree by the time the
   * review re-renders.
   *
   * Omit on a surface that cannot edit the document (a read-only history view).
   * Remove is still offered there — the verdict is worth recording — but the
   * entry then renders as `markedForRemoval`, never as finished.
   */
  onRemoveEvidence?: (evidence: string) => void | Promise<void>;
  /** The entry whose verdict is in flight — its two buttons show as busy and
   *  every other row is locked (one write per run at a time). */
  resolvingIssueKey?: string | null;
  resolveError?: string | null;
}

/**
 * The terminal per-bullet review: every claim the deterministic checks could
 * not trace back to the source résumé, each awaiting Remove or Keep.
 *
 * **Nothing is removed silently, and "Remove" removes.** Both halves matter:
 * the deletion only ever happens on an explicit click, and when it happens it
 * is a real edit to the document the user is looking at (`onRemoveEvidence` →
 * the host's edit/save path) followed by the recorded verdict
 * (`resolveFabrication`). A verdict on its own is intent, not fact — so an
 * entry counts as finished only when the document AGREES with it, and a
 * recorded-but-unapplied removal renders its own state instead of a tick.
 *
 * An entry whose evidence no longer occurs in the document is shown as such
 * rather than as a live "judge this line" prompt: the user may have edited it
 * away, or a "Re-check" preserved the entry across a newer document (preserved
 * deliberately — dropping it strands the run at `needsReview` forever). It is
 * still decidable, because deciding it is what clears the review.
 */
export function FabricationReview({
  entries,
  documentText,
  onResolve,
  onRemoveEvidence,
  resolvingIssueKey,
  resolveError,
}: FabricationReviewProps) {
  const { t } = useTranslation();
  if (entries.length === 0) return null;

  const pending = unresolvedCount(entries, documentText);
  const busy = !!resolvingIssueKey;

  /**
   * Apply first, record second — and record even when the apply fails.
   *
   * The user's verdict is never thrown away: a failed (or impossible) edit
   * leaves the span in the document, which is precisely what makes
   * `presentFabrication` return `markedForRemoval` and keeps the integrity chip
   * off green. There is no state to invent here; the document is the evidence.
   */
  const handleRemove = async (entry: Fabrication) => {
    if (onRemoveEvidence) {
      try {
        await onRemoveEvidence(entry.evidence);
      } catch (err) {
        console.error('[FabricationReview] applying a Remove to the document failed', {
          issueKey: entry.issueKey,
          error: errorDetail(err),
        });
      }
    }
    onResolve?.(entry.issueKey, 'remove');
  };

  return (
    <div className="border-t border-white/[0.06] pt-4">
      <h3 className="mb-1 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
        <ShieldAlert size={11} className="text-amber-400" />
        {t('quality.panel.review.title')}
      </h3>
      <p className="mb-2 text-[10px] leading-relaxed text-foreground/45">
        {t('quality.panel.review.description')}
      </p>
      <p
        role="status"
        className={
          pending > 0
            ? 'mb-2 text-[10px] font-medium text-amber-400'
            : 'mb-2 text-[10px] font-medium text-emerald-400'
        }
      >
        {pending > 0
          ? t('quality.panel.review.pending', { count: pending })
          : t('quality.panel.review.allResolved')}
      </p>

      <ul className="space-y-2">
        {entries.map((entry) => {
          const presentation = presentFabrication(entry, documentText);
          const resolving = resolvingIssueKey === entry.issueKey;
          return (
            <li
              key={entry.issueKey}
              className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2.5"
            >
              {presentation === 'orphaned' && (
                <p className="mb-1 flex items-center gap-1.5 text-[10px] font-medium text-blue-400">
                  <FileQuestion size={10} className="shrink-0" />
                  {t('quality.panel.review.orphaned')}
                </p>
              )}
              <blockquote className="border-l-2 border-white/10 pl-2 text-[11px] italic text-foreground/60">
                “{entry.evidence}”
              </blockquote>
              {presentation === 'orphaned' && (
                <p className="mt-1 text-[10px] leading-relaxed text-foreground/35">
                  {t('quality.panel.review.orphanedHint')}
                </p>
              )}

              {/* A Remove that never reached the document: the verdict is on
                  record, the line is still there, and saying "Removed" would be
                  the lie. Names what is left to do instead. */}
              {presentation === 'markedForRemoval' ? (
                <div className="mt-2">
                  <p className="text-[10px] font-medium text-amber-400">
                    {t('quality.panel.review.removalPending')}
                  </p>
                  <p className="mt-0.5 text-[10px] leading-relaxed text-foreground/35">
                    {t('quality.panel.review.removalPendingHint')}
                  </p>
                </div>
              ) : entry.decision ? (
                <p className="mt-2 text-[10px] font-medium text-foreground/55">
                  {entry.decision === 'remove'
                    ? t('quality.panel.review.removed')
                    : t('quality.panel.review.kept')}
                </p>
              ) : (
                onResolve && (
                  <div className="mt-2 flex gap-1.5">
                    <Button
                      type="button"
                      variant="danger"
                      size="sm"
                      disabled={busy}
                      loading={resolving}
                      onClick={() => void handleRemove(entry)}
                      className="text-[10px]"
                    >
                      <Trash2 size={10} />
                      {t('quality.panel.review.remove')}
                    </Button>
                    <Button
                      type="button"
                      variant="glass"
                      size="sm"
                      disabled={busy}
                      loading={resolving}
                      onClick={() => onResolve(entry.issueKey, 'keep')}
                      className="text-[10px]"
                    >
                      <Check size={10} />
                      {t('quality.panel.review.keep')}
                    </Button>
                  </div>
                )
              )}
            </li>
          );
        })}
      </ul>

      {resolveError && (
        <p role="alert" className="mt-2 text-[10px] leading-relaxed text-red-400">
          {resolveError}
        </p>
      )}
    </div>
  );
}
