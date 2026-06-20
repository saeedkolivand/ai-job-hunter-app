import { Loader2, Search, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef } from 'react';

import type { BoardAuthRequirement } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, CardSkeleton, cn, GlassCard, Input, transition } from '@ajh/ui';

import { makeRovingTabindex } from '@/hooks/use-roving-tabindex';
import { useBoardsCatalog } from '@/services/use-boards';

import { AuthHint } from './AuthHint';
import { AuthModeBadge } from './AuthModeBadge';
import type { ScrapeFormState } from './constants';
import { ScrapeFilters } from './ScrapeFilters';

interface ScrapeFormProps {
  show: boolean;
  form: ScrapeFormState;
  scraping: boolean;
  scrapeOutcome: { ok: boolean; note?: string } | null;
  boardConnected: boolean;
  connectPending: boolean;
  disconnectPending: boolean;
  onToggle: () => void;
  onFormChange: (updates: Partial<ScrapeFormState>) => void;
  onStart: () => void;
  onCancel: () => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onGeocode: (query: string) => Promise<{ display: string }[]>;
}

export function ScrapeForm({
  show,
  form,
  scraping,
  scrapeOutcome,
  boardConnected,
  connectPending,
  disconnectPending,
  onToggle,
  onFormChange,
  onStart,
  onCancel,
  onConnect,
  onDisconnect,
  onGeocode,
}: ScrapeFormProps) {
  const { t } = useTranslation();
  const boardRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const { data: catalogRaw, isLoading: catalogLoading } = useBoardsCatalog();
  const listedBoards = (catalogRaw ?? []).filter((e) => e.listed);

  // Normalize: if the persisted board ID is no longer listed, reset to the first listed board.
  const selectedEntry = listedBoards.find((e) => e.id === form.board);
  const effectiveBoardId = selectedEntry?.id ?? listedBoards[0]?.id;
  useEffect(() => {
    if (!catalogLoading && listedBoards.length > 0 && !selectedEntry && effectiveBoardId) {
      onFormChange({ board: effectiveBoardId });
    }
  }, [catalogLoading, listedBoards, selectedEntry, effectiveBoardId, onFormChange]);

  // Derive the selected board's auth tier from the catalog; default to 'guest'.
  const selectedAuth: BoardAuthRequirement = (selectedEntry ?? listedBoards[0])?.auth ?? 'guest';

  // Badge shows for optional/required; guest boards show nothing.
  const showAuthBadge = selectedAuth !== 'guest';
  // AuthHint (settings nudge) only for optional boards when not connected.
  const showAuthHint = selectedAuth === 'optional' && !boardConnected;

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
                  if (e.key === 'Enter' && !scraping && form.query.trim()) {
                    e.preventDefault();
                    onStart();
                  }
                }}
                placeholder={t('jobs.queryPlaceholder')}
                disabled={scraping}
                allowClear
                className="w-full bg-white/[0.03] text-sm text-foreground placeholder:text-foreground/25 disabled:opacity-50"
              />
            </div>

            {/* Board picker */}
            <div className="mb-4">
              <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
                {t('jobs.board')}
              </div>
              {catalogLoading ? (
                <CardSkeleton className="h-8 w-full" />
              ) : (
                <div
                  role="radiogroup"
                  aria-label={t('jobs.board')}
                  className="flex flex-wrap gap-1.5"
                  onKeyDown={
                    scraping
                      ? undefined
                      : makeRovingTabindex(
                          listedBoards.map((b) => b.id),
                          form.board,
                          (id) => onFormChange({ board: id }),
                          boardRefs
                        )
                  }
                >
                  {listedBoards.map(({ id }, i) => {
                    const active = effectiveBoardId === id;
                    return (
                      <Button
                        key={id}
                        ref={(el) => {
                          boardRefs.current[i] = el;
                        }}
                        role="radio"
                        aria-checked={active}
                        tabIndex={active ? 0 : -1}
                        variant="ghost"
                        disabled={scraping}
                        onClick={() => onFormChange({ board: id })}
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

            <AuthModeBadge
              show={showAuthBadge}
              board={form.board}
              auth={selectedAuth}
              boardConnected={boardConnected}
              disconnectPending={disconnectPending}
              connectPending={connectPending}
              onDisconnect={onDisconnect}
              onConnect={onConnect}
            />

            <AuthHint show={showAuthHint} connectPending={connectPending} onConnect={onConnect} />

            <ScrapeFilters
              form={form}
              scraping={scraping}
              boardConnected={boardConnected}
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
                  {t('jobs.scraping')} {form.board}…
                </div>
              </div>
            )}

            {/* Footer */}
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
                      scrapeOutcome.ok ? 'text-emerald-400/70' : 'text-amber-400/70'
                    )}
                  >
                    {scrapeOutcome.ok ? t('jobs.done') : (scrapeOutcome.note ?? t('jobs.failed'))}
                  </span>
                )
              )}
              <Button
                variant="primary"
                onClick={() => onStart()}
                disabled={scraping || !form.query.trim()}
                loading={scraping}
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
