import { AlertOctagon, AlertTriangle, CheckCircle2, History, RefreshCw, X } from 'lucide-react';
import { useMemo } from 'react';

import type { ContentReportPayload } from '@ajh/shared/ipc';
import { useTranslation } from '@ajh/translations';
import { Button, EmptyState, ModalShell } from '@ajh/ui';

type ContentIssue = ContentReportPayload['issues'][number];

export interface QualityReportPanelProps {
  open: boolean;
  onClose: () => void;
  /** This document's portion of the quality report; `null`/`undefined` renders
   *  the same empty state as a report with zero issues. */
  report: ContentReportPayload | null | undefined;
  /** Which document `report` describes — drives the modal title. */
  docKind: 'resume' | 'coverLetter';
  /** True when the document has changed since this report was generated —
   *  shows a small notice; the issues below still describe an EARLIER version
   *  of the text. */
  stale?: boolean;
  /** Re-run validation against the current text — clears staleness on success.
   *  Omit to hide the action. */
  onRecheck?: () => void;
  rechecking?: boolean;
}

/** Sentinel grouping key for document-wide findings (`issue.section === null`) —
 *  distinguishable from any real section name, which is always a string. */
const DOCUMENT_WIDE = Symbol('document-wide');
type SectionKey = string | typeof DOCUMENT_WIDE;

/** Group issues by section, named sections first (most actionable) and the
 *  document-wide bucket last (reads like a footnote), each in emission order. */
function groupBySection(issues: ContentIssue[]): [SectionKey, ContentIssue[]][] {
  const map = new Map<SectionKey, ContentIssue[]>();
  for (const issue of issues) {
    const key: SectionKey = issue.section ?? DOCUMENT_WIDE;
    const existing = map.get(key);
    if (existing) existing.push(issue);
    else map.set(key, [issue]);
  }
  const named = [...map.entries()].filter(([key]) => key !== DOCUMENT_WIDE);
  const wide = map.get(DOCUMENT_WIDE);
  return wide ? [...named, [DOCUMENT_WIDE, wide]] : named;
}

/**
 * Codes whose Rust-authored `message` carries an interpolated NUMBER (a count,
 * percentage, or threshold `validate::content` computed) that is NOT also
 * repeated in the issue's separate `evidence` field — so translating them to
 * the static `quality.issue.<code>` string would silently throw that number
 * away. Audited against every `issue(...)` call site in
 * `apps/desktop/src-tauri/src/validate/content/*.rs`; a code missing from this
 * list either has no computed number in its message, or `evidence` already
 * carries it (e.g. `alignment.low_coverage`'s two percentages are duplicated
 * into `evidence`, so its translation stays preferred).
 */
const CODES_WITH_LOSSY_NUMBERS = new Set<string>([
  // "<term>" appears N times in TOTAL content words — evidence is only "term ×N", missing TOTAL.
  'ats.keyword_density',
  // This bullet is N characters — evidence quotes the bullet text, never the count.
  'ats.long_bullet',
  // "<role>" has N bullets — evidence is only the role name.
  'ats.bullet_count',
  // stddev across N sentences — evidence carries the stddev only, not N.
  'voice.low_burstiness',
  // T of N sentences are triplets — evidence is a normalized per-10 rate, not the raw T/N.
  'voice.rule_of_three_density',
]);

/**
 * Full content-quality report for one generated document: issues grouped by
 * section with a severity chip + guidance-framed message + quoted evidence,
 * and a metrics footer. Never scores or verdicts the CANDIDATE — every message
 * advises on the document (job-match-standards framing), matching the Rust
 * validator's own posture (`validate::content`).
 *
 * A code in [`CODES_WITH_LOSSY_NUMBERS`] renders its own Rust-authored
 * `message` instead of the translation, so the number it computed survives.
 * Otherwise: a code with no matching `quality.issue.<code>` translation (a
 * future Rust check this build predates) falls back to the issue's own
 * Rust-authored `message` when present, and only to the generic
 * `quality.fallback` when that's also empty — never a raw i18n key.
 */
export function QualityReportPanel({
  open,
  onClose,
  report,
  docKind,
  stale,
  onRecheck,
  rechecking,
}: QualityReportPanelProps) {
  const { t, i18n } = useTranslation();
  const titleId = 'quality-report-title';

  const groups = useMemo(() => groupBySection(report?.issues ?? []), [report]);
  const metrics = report?.metrics;

  const messageFor = (code: string, message: string) => {
    if (CODES_WITH_LOSSY_NUMBERS.has(code) && message) return message;
    const key = `quality.issue.${code}`;
    if (i18n.exists(key)) return t(key);
    return message || t('quality.fallback');
  };

  return (
    <ModalShell
      open={open}
      onClose={onClose}
      maxWidth="max-w-2xl"
      ariaLabelledby={titleId}
      header={
        <div className="flex items-center justify-between border-b border-white/[0.08] px-5 py-4">
          <h2 id={titleId} className="text-sm font-semibold text-foreground/90">
            {t(
              docKind === 'resume' ? 'quality.panel.resumeTitle' : 'quality.panel.coverLetterTitle'
            )}
          </h2>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onClose}
            aria-label={t('common.close')}
            className="h-7 w-7 p-0"
          >
            <X size={14} />
          </Button>
        </div>
      }
    >
      <div className="space-y-5 px-5 py-4">
        {stale && (
          <div className="flex items-center justify-between gap-3 rounded-lg border border-blue-400/20 bg-blue-400/5 px-3 py-2 text-[11px] text-blue-300/90">
            <span className="flex items-center gap-1.5">
              <History size={12} className="shrink-0" />
              {t('quality.panel.staleNotice')}
            </span>
            {onRecheck && (
              <Button
                type="button"
                variant="info"
                size="sm"
                onClick={onRecheck}
                disabled={rechecking}
                className="shrink-0"
              >
                <RefreshCw size={11} className={rechecking ? 'animate-spin' : undefined} />
                {rechecking ? t('quality.panel.rechecking') : t('quality.panel.recheck')}
              </Button>
            )}
          </div>
        )}
        {groups.length === 0 ? (
          <EmptyState
            icon={CheckCircle2}
            title={t('quality.panel.emptyTitle')}
            description={t('quality.panel.emptyDescription')}
          />
        ) : (
          groups.map(([key, issues]) => (
            <div key={typeof key === 'string' ? key : 'document-wide'}>
              <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
                {typeof key === 'string' ? key : t('quality.panel.documentWide')}
              </h3>
              <ul className="space-y-2">
                {issues.map((issue, i) => (
                  <li
                    key={`${issue.code}-${i}`}
                    className="rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2.5"
                  >
                    {issue.severity === 'critical' ? (
                      <span className="inline-flex items-center gap-1 rounded-full bg-red-400/10 px-2 py-0.5 text-[10px] font-semibold text-red-300">
                        <AlertOctagon size={10} /> {t('quality.panel.critical')}
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1 rounded-full bg-amber-400/10 px-2 py-0.5 text-[10px] font-semibold text-amber-300">
                        <AlertTriangle size={10} /> {t('quality.panel.warning')}
                      </span>
                    )}
                    <p className="mt-1.5 text-xs text-foreground/70">
                      {messageFor(issue.code, issue.message)}
                    </p>
                    {issue.evidence && (
                      <blockquote className="mt-1.5 border-l-2 border-white/10 pl-2 text-[11px] italic text-foreground/45">
                        “{issue.evidence}”
                      </blockquote>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          ))
        )}

        {metrics && (
          <div className="border-t border-white/[0.06] pt-4">
            <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-foreground/45">
              {t('quality.panel.metrics.title')}
            </h3>
            <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-[11px] text-foreground/60">
              <div className="flex items-center justify-between gap-2">
                <dt>{t('quality.panel.metrics.keywordCoverage')}</dt>
                <dd className="font-medium text-foreground/85">
                  {metrics.keywordCoverage != null
                    ? `${Math.round(metrics.keywordCoverage)}%`
                    : '—'}
                </dd>
              </div>
              {/* The CoverLetter arm of validate_content returns these three as
                  hard constants, not measurements (alignment/duplicates never
                  run for a letter) — rendering them would claim "0 requirements
                  covered" as fact. Only keywordCoverage is genuinely computed
                  for both kinds. */}
              {docKind === 'resume' && (
                <div className="flex items-center justify-between gap-2">
                  <dt>{t('quality.panel.metrics.topRequirementHits')}</dt>
                  <dd className="font-medium text-foreground/85">
                    {metrics.topRequirementHits ?? '—'}
                  </dd>
                </div>
              )}
              {docKind === 'resume' && (
                <div className="flex items-center justify-between gap-2">
                  <dt>{t('quality.panel.metrics.duplicateRatio')}</dt>
                  <dd className="font-medium text-foreground/85">
                    {Math.round(metrics.duplicateRatio * 100)}%
                  </dd>
                </div>
              )}
              {docKind === 'resume' && (
                <div className="flex items-center justify-between gap-2">
                  <dt>{t('quality.panel.metrics.roles')}</dt>
                  <dd className="font-medium text-foreground/85">
                    {metrics.rolesSource} → {metrics.rolesOutput}
                  </dd>
                </div>
              )}
            </dl>
          </div>
        )}
      </div>
    </ModalShell>
  );
}
