import { Check, FileQuestion, ShieldAlert, Trash2 } from 'lucide-react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { type Fabrication, presentFabrication, unresolvedCount } from '@/lib/generate';

export interface FabricationReviewProps {
  entries: Fabrication[];
  /** The document as it stands NOW — decides whether an entry's evidence can
   *  still be found (see the `orphaned` presentation). */
  documentText: string;
  /** Record one verdict. Omit to render the list read-only (an older run). */
  onResolve?: (issueKey: string, decision: 'remove' | 'keep') => void;
  /** The entry whose verdict is in flight — its two buttons show as busy and
   *  every other row is locked (one write per run at a time). */
  resolvingIssueKey?: string | null;
  resolveError?: string | null;
}

/**
 * The terminal per-bullet review: every claim the deterministic checks could
 * not trace back to the source résumé, each awaiting Remove or Keep.
 *
 * **Nothing is removed silently** — that is the whole point of the panel. A run
 * stays `needsReview` until every entry carries a verdict, and the verdict is
 * recorded on the persisted report (`resolveFabrication`), not applied to the
 * text behind the user's back.
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
  resolvingIssueKey,
  resolveError,
}: FabricationReviewProps) {
  const { t } = useTranslation();
  if (entries.length === 0) return null;

  const pending = unresolvedCount(entries);
  const busy = !!resolvingIssueKey;

  return (
    <div className="border-t border-white/[0.06] pt-4">
      <h3 className="mb-1 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
        <ShieldAlert size={11} className="text-amber-400/80" />
        {t('quality.panel.review.title')}
      </h3>
      <p className="mb-2 text-[10px] leading-relaxed text-foreground/45">
        {t('quality.panel.review.description')}
      </p>
      <p
        role="status"
        className={
          pending > 0
            ? 'mb-2 text-[10px] font-medium text-amber-300/85'
            : 'mb-2 text-[10px] font-medium text-emerald-300/85'
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
                <p className="mb-1 flex items-center gap-1.5 text-[10px] font-medium text-blue-300/85">
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

              {entry.decision ? (
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
                      onClick={() => onResolve(entry.issueKey, 'remove')}
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
        <p role="alert" className="mt-2 text-[10px] leading-relaxed text-red-300/80">
          {resolveError}
        </p>
      )}
    </div>
  );
}
