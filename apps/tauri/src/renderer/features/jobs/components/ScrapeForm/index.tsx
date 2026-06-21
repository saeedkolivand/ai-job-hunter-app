import { Loader2, Search, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { flushSync } from 'react-dom';

import type { BoardCatalogEntry } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, CardSkeleton, cn, GlassCard, Input, transition } from '@ajh/ui';

import { AUTH_BENEFITS } from '@/features/jobs/constants';
import { makeMultiSelectKeyHandler } from '@/hooks/use-roving-tabindex';
import { useBoardsCatalog, useBoardStatuses } from '@/services/use-boards';

import { BoardConnectChip } from './BoardConnectChip';
import type { ScrapeFormState } from './constants';
import { ScrapeFilters } from './ScrapeFilters';

interface ScrapeFormProps {
  show: boolean;
  form: ScrapeFormState;
  scraping: boolean;
  scrapeOutcome: { ok: boolean; note?: string } | null;
  onToggle: () => void;
  onFormChange: (updates: Partial<ScrapeFormState>) => void;
  onStart: () => void;
  onCancel: () => void;
  onGeocode: (query: string) => Promise<{ display: string }[]>;
}

/** Toggle membership of `id` in the array without mutation. */
function toggleBoard(boards: string[], id: string): string[] {
  return boards.includes(id) ? boards.filter((b) => b !== id) : [...boards, id];
}

export function ScrapeForm({
  show,
  form,
  scraping,
  scrapeOutcome,
  onToggle,
  onFormChange,
  onStart,
  onCancel,
  onGeocode,
}: ScrapeFormProps) {
  const { t } = useTranslation();
  const boardRefs = useRef<(HTMLButtonElement | null)[]>([]);
  // Tracks keyboard-focus position independently of the selection set (multi-select pattern).
  const focusedBoardIdx = useRef<number>(0);
  // Raw string buffer for the companies input. Parse to string[] only on blur so
  // mid-typing state (e.g. "stripe, ") is never clobbered by join(', ').
  const [rawCompanies, setRawCompanies] = useState(() => (form.companies ?? []).join(', '));

  const { data: catalogRaw, isLoading: catalogLoading } = useBoardsCatalog();
  const listedBoards: BoardCatalogEntry[] = (catalogRaw ?? []).filter((e) => e.listed);

  // Normalize: ensure every persisted id in form.boards still exists in the
  // catalog; if none remain, default to the first listed board.
  // Guard: only call onFormChange when the normalized set actually differs to
  // prevent an infinite re-render loop.
  useEffect(() => {
    if (catalogLoading || listedBoards.length === 0) return;
    const listedIds = new Set(listedBoards.map((e) => e.id));
    const valid = form.boards.filter((id) => listedIds.has(id));
    const needsUpdate = valid.length !== form.boards.length || form.boards.length === 0;
    if (!needsUpdate) return;
    const fallback = listedBoards[0]?.id ?? '';
    onFormChange({ boards: valid.length > 0 ? valid : fallback ? [fallback] : [] });
  }, [catalogLoading, listedBoards, form.boards, onFormChange]);

  const selectedSet = new Set(form.boards);
  const allSelected = listedBoards.length > 0 && listedBoards.every((e) => selectedSet.has(e.id));

  // Boards that are selected and require login, filtered against catalog auth.
  const needsLoginBoards = listedBoards.filter(
    (e) => selectedSet.has(e.id) && (e.auth === 'optional' || e.auth === 'required')
  );

  // True when any currently-selected board requires a company slug (ATS boards).
  // Derived entirely from catalog metadata — no hardcoded board list.
  const showCompanyInput = listedBoards.some((e) => selectedSet.has(e.id) && e.requiresCompany);

  // When the field disappears (no ATS board selected), clear the raw buffer and
  // reset the parent's companies array so a stale list isn't sent on next scrape.
  // Skip mount: only clear on a transition from visible → hidden (not on initial render).
  const onFormChangeRef = useRef(onFormChange);
  onFormChangeRef.current = onFormChange;
  const prevShowCompanyRef = useRef(showCompanyInput);
  useEffect(() => {
    const wasShowing = prevShowCompanyRef.current;
    prevShowCompanyRef.current = showCompanyInput;
    if (showCompanyInput || !wasShowing) return; // still visible, or never was
    setRawCompanies('');
    onFormChangeRef.current({ companies: [] });
  }, [showCompanyInput]);

  // Query connection status for all selected auth-benefit boards via a service hook.
  const authBenefitBoardIds = form.boards.filter((b) => AUTH_BENEFITS.has(b));
  const { anyConnected: anyAuthBenefitConnected } = useBoardStatuses(authBenefitBoardIds);

  // Boards with auth === 'required' that are selected — must be connected to start.
  const requiredBoardIds = listedBoards
    .filter((e) => selectedSet.has(e.id) && e.auth === 'required')
    .map((e) => e.id);
  const { results: requiredResults } = useBoardStatuses(requiredBoardIds);
  const unconnectedRequired = requiredBoardIds.filter(
    (_id, i) =>
      (requiredResults[i]?.data as { connected?: boolean } | undefined)?.connected !== true
  );
  const blockedByRequiredLogin = unconnectedRequired.length > 0;

  const handleSelectAll = () => {
    onFormChange({ boards: listedBoards.map((e) => e.id) });
  };
  const handleClear = () => {
    // Always keep at least one; clear to the first listed board.
    const first = listedBoards[0]?.id;
    if (first) onFormChange({ boards: [first] });
  };

  // Count label: "3 selected" — i18next picks the plural form automatically.
  const countLabel = t('jobs.boardsSelected', { count: form.boards.length });

  // Progress label
  const scrapingLabel =
    form.boards.length === 1
      ? (catalogRaw?.find((e) => e.id === form.boards[0])?.displayName ?? form.boards[0])
      : countLabel;

  return (
    <AnimatePresence>
      {show && (
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -8 }}
          transition={transition.normal}
          className="mb-4"
        >
          <GlassCard className="p-5">
            {/* Header */}
            <div className="mb-4 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <div className="flex h-5 w-5 items-center justify-center rounded-md bg-brand/15">
                  <Search size={11} className="text-brand-soft" />
                </div>
                <span className="text-xs font-medium text-foreground/70">
                  {t('jobs.newScrape')}
                </span>
              </div>
              <Button
                variant="ghost"
                aria-label={t('common.close')}
                onClick={onToggle}
                className="rounded-md p-1 text-foreground/40 hover:bg-white/5 hover:text-foreground/70 h-auto"
              >
                <X size={13} />
              </Button>
            </div>

            {/* Query — hero input */}
            <div className="mb-4">
              <Input
                type="text"
                value={form.query}
                onChange={(e) => onFormChange({ query: e.target.value })}
                onKeyDown={(e) => {
                  if (
                    e.key === 'Enter' &&
                    !scraping &&
                    !blockedByRequiredLogin &&
                    form.query.trim()
                  ) {
                    e.preventDefault();
                    if (showCompanyInput) {
                      const parsed = rawCompanies
                        .split(',')
                        .map((s) => s.trim())
                        .filter(Boolean);
                      flushSync(() => {
                        onFormChange({ companies: parsed });
                      });
                    }
                    onStart();
                  }
                }}
                placeholder={t('jobs.queryPlaceholder')}
                disabled={scraping}
                allowClear
                className="w-full bg-white/[0.03] text-sm text-foreground placeholder:text-foreground/25 disabled:opacity-50"
              />
            </div>

            {/* Board picker — multi-select toggle group */}
            <div className="mb-4">
              <div className="mb-2 flex items-center gap-2">
                <span className="text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
                  {t('jobs.board')}
                </span>
                {!catalogLoading && listedBoards.length > 0 && (
                  <>
                    <span
                      aria-live="polite"
                      aria-atomic="true"
                      className="rounded-full bg-brand/20 px-1.5 py-px text-[10px] font-medium text-brand-soft"
                    >
                      {countLabel}
                    </span>
                    <div className="ml-auto flex items-center gap-1">
                      <Button
                        variant="ghost"
                        disabled={scraping || allSelected}
                        onClick={handleSelectAll}
                        className="h-auto rounded px-1.5 py-1 text-[10px] text-foreground/50 hover:text-foreground/80 disabled:opacity-40"
                      >
                        {t('jobs.selectAll')}
                      </Button>
                      <Button
                        variant="ghost"
                        disabled={scraping || form.boards.length <= 1}
                        onClick={handleClear}
                        className="h-auto rounded px-1.5 py-1 text-[10px] text-foreground/50 hover:text-foreground/80 disabled:opacity-40"
                      >
                        {t('jobs.clearBoards')}
                      </Button>
                    </div>
                  </>
                )}
              </div>
              {catalogLoading ? (
                <CardSkeleton className="h-8 w-full" />
              ) : (
                <div
                  role="group"
                  aria-label={t('jobs.board')}
                  className="flex flex-wrap gap-1.5"
                  onKeyDown={
                    scraping
                      ? undefined
                      : makeMultiSelectKeyHandler(
                          listedBoards.length,
                          focusedBoardIdx,
                          boardRefs,
                          (idx) => {
                            const id = listedBoards[idx]?.id;
                            if (!id) return;
                            // Prevent deselecting the last board.
                            if (selectedSet.has(id) && form.boards.length === 1) return;
                            onFormChange({ boards: toggleBoard(form.boards, id) });
                          }
                        )
                  }
                >
                  {listedBoards.map(({ id }, i) => {
                    const active = selectedSet.has(id);
                    return (
                      <Button
                        key={id}
                        ref={(el) => {
                          boardRefs.current[i] = el;
                        }}
                        aria-pressed={active}
                        tabIndex={i === focusedBoardIdx.current ? 0 : -1}
                        variant="ghost"
                        disabled={scraping}
                        onClick={() => {
                          // Prevent deselecting the last board.
                          if (active && form.boards.length === 1) return;
                          focusedBoardIdx.current = i;
                          onFormChange({ boards: toggleBoard(form.boards, id) });
                        }}
                        className={cn(
                          'rounded-lg px-2.5 py-1 text-[11px] transition-all',
                          active
                            ? 'bg-brand/20 text-brand-soft ring-1 ring-brand/40'
                            : 'bg-white/[0.04] text-foreground/50 hover:bg-white/[0.07] hover:text-foreground/80',
                          'disabled:cursor-not-allowed disabled:opacity-40'
                        )}
                      >
                        {t(`jobs.boards.${id}`)}
                      </Button>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Auth affordance — compact "needs login" row per selected board */}
            {needsLoginBoards.length > 0 && (
              <div className="mb-3 flex flex-wrap items-center gap-1.5">
                <span className="text-[10px] text-foreground/55">{t('jobs.needsLogin.label')}</span>
                {needsLoginBoards.map((e) => (
                  <BoardConnectChip key={e.id} board={e.id} required={e.auth === 'required'} />
                ))}
              </div>
            )}

            {/* Companies input — only shown when an ATS board (requiresCompany) is selected */}
            {showCompanyInput && (
              <div className="mb-4">
                <label
                  htmlFor="scrape-companies"
                  className="mb-1.5 block text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55"
                >
                  {t('jobs.companies.label')}
                </label>
                <Input
                  id="scrape-companies"
                  aria-describedby="scrape-companies-hint"
                  type="text"
                  value={rawCompanies}
                  onChange={(e) => setRawCompanies(e.target.value)}
                  onBlur={(e) => {
                    // Parse to string[] only on blur so mid-typing state
                    // (e.g. "stripe, ") is never snapped back by join(', ').
                    const parsed = e.target.value
                      .split(',')
                      .map((s) => s.trim())
                      .filter(Boolean);
                    onFormChange({ companies: parsed });
                  }}
                  placeholder={t('jobs.companies.placeholder')}
                  disabled={scraping}
                  allowClear
                  className="w-full bg-white/[0.03] text-sm text-foreground placeholder:text-foreground/25 disabled:opacity-50"
                />
                <p id="scrape-companies-hint" className="mt-1 text-[10px] text-foreground/40">
                  {t('jobs.companies.hint')}
                </p>
              </div>
            )}

            <ScrapeFilters
              form={form}
              scraping={scraping}
              boardConnected={anyAuthBenefitConnected}
              onFormChange={onFormChange}
              onGeocode={onGeocode}
            />

            {/* Progress bar */}
            {scraping && (
              <div className="mb-4">
                <div className="h-px w-full overflow-hidden rounded-full bg-white/[0.06]">
                  <motion.div
                    className="h-full rounded-full bg-gradient-to-r from-brand to-brand-soft"
                    initial={{ width: '0%' }}
                    animate={{ width: '85%' }}
                    transition={transition.fakeProgress}
                  />
                </div>
                <div className="mt-1.5 flex items-center gap-1.5 text-[11px] text-foreground/40">
                  <Loader2 size={10} className="animate-spin" />
                  {t('jobs.scraping')} {scrapingLabel}…
                </div>
              </div>
            )}

            {/* Footer */}
            {!scraping && blockedByRequiredLogin && (
              <p
                id="scrape-blocked-hint"
                aria-live="polite"
                className="mb-2 text-[11px] text-amber-400/70"
              >
                {t('jobs.needsLogin.blockedHint', {
                  boards: unconnectedRequired.map((id) => t(`jobs.boards.${id}`)).join(', '),
                })}
              </p>
            )}
            <div className="flex items-center justify-end gap-2">
              {scraping ? (
                <Button variant="ghost" onClick={onCancel}>
                  {t('jobs.cancel')}
                </Button>
              ) : (
                scrapeOutcome && (
                  <span
                    className={cn(
                      'text-[11px]',
                      scrapeOutcome.ok && !scrapeOutcome.note
                        ? 'text-emerald-400/70'
                        : scrapeOutcome.ok && scrapeOutcome.note
                          ? 'text-amber-400/70'
                          : 'text-amber-400/70'
                    )}
                  >
                    {scrapeOutcome.ok
                      ? (scrapeOutcome.note ?? t('jobs.done'))
                      : (scrapeOutcome.note ?? t('jobs.failed'))}
                  </span>
                )
              )}
              <Button
                variant="primary"
                onClick={() => {
                  if (showCompanyInput) {
                    // Flush the raw buffer before starting — guarantees an
                    // un-blurred entry is not lost (React 18 batches state
                    // updates so flushSync forces the parent to re-render with
                    // the parsed companies before startScrape captures them).
                    const parsed = rawCompanies
                      .split(',')
                      .map((s) => s.trim())
                      .filter(Boolean);
                    flushSync(() => {
                      onFormChange({ companies: parsed });
                    });
                  }
                  onStart();
                }}
                disabled={scraping || !form.query.trim() || blockedByRequiredLogin}
                loading={scraping}
                aria-describedby={
                  !scraping && blockedByRequiredLogin ? 'scrape-blocked-hint' : undefined
                }
                data-testid="scrape-start-button"
                className="transition-all duration-150 ease-out"
              >
                {!scraping && <Search size={12} />}
                {scraping ? t('jobs.scraping') : t('jobs.startScrape')}
              </Button>
            </div>
          </GlassCard>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
