import { Scissors } from 'lucide-react';

import { useTranslation } from '@ajh/translations';

import { useTrimSuggestions } from '@/services/use-match';

interface TrimPanelProps {
  /** Committed résumé text — the same string the preview rendered. */
  resumeText: string;
  /** Raw job ad. Empty disables the panel: there is nothing to rank against. */
  jobText: string;
  /** Rendered page count, from the preview. */
  pages: number;
  /** Export market — resolves the market's customary length. */
  locale?: string;
}

/** How many suggestions to show before the list stops being advice and starts being a chore. */
const VISIBLE = 6;

/**
 * Shortest document that could possibly overflow any market's target. No
 * `LocaleProfile` sets `max_pages` below 2, so a 1–2 page résumé can never be
 * over — pinned by `no_market_targets_fewer_than_two_pages` in
 * `commands/match_resume.rs`. Used only to skip the query; the real comparison
 * is against the market's own `maxPages` below.
 */
const SHORTEST_OVERFLOW = 2;

/**
 * Advisory "this is running long" panel: when the rendered résumé exceeds the
 * export market's customary page count, rank its bullets by how little of THIS
 * posting's vocabulary each one carries and show the weakest first.
 *
 * Read-only by design — it never edits the document. Removing a line the user
 * spent a career earning is their call, made in the editor beside this panel.
 *
 * `ponytail:` native `<details>` rather than a state-driven disclosure — the
 * platform already ships open/close, keyboard support, and the a11y semantics.
 */
export function TrimPanel({ resumeText, jobText, pages, locale }: TrimPanelProps) {
  const { t } = useTranslation();
  // A document short enough that no market could call it long never runs the
  // query at all — the common case costs nothing.
  const { data } = useTrimSuggestions(resumeText, jobText, locale, pages > SHORTEST_OVERFLOW);

  if (!data || pages <= data.maxPages) return null;

  const { maxPages } = data;

  const lines = data.lines.slice(0, VISIBLE);

  return (
    <details className="shrink-0 border-t border-amber-400/20 bg-amber-400/5 px-6 py-2">
      <summary className="flex cursor-pointer list-none items-center gap-2 text-[11px] text-amber-400/80">
        <Scissors size={11} className="shrink-0" />
        {lines.length > 0
          ? t('aiGenerate.trim.summary', { count: lines.length, pages, maxPages })
          : t('aiGenerate.trim.noKeywords')}
      </summary>

      {lines.length > 0 && (
        <>
          <p className="mt-2 text-[10px] text-foreground/35">{t('aiGenerate.trim.hint')}</p>
          <ul className="mt-1.5 flex flex-col gap-1.5 pb-1">
            {lines.map((line) => (
              <li
                key={line.text}
                className="flex items-baseline gap-2 text-[11px] text-foreground/60"
              >
                <span className="min-w-0 flex-1 truncate">{line.text}</span>
                {line.hits.length > 0 && (
                  <span className="shrink-0 text-[10px] text-foreground/30">
                    {line.hits.join(' · ')}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
    </details>
  );
}
