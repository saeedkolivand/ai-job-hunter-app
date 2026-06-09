import { Loader2, Search, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import { Button, cn, GlassCard, Input, transition } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

import { AuthHint } from './AuthHint';
import { AuthModeBadge } from './AuthModeBadge';
import { AUTH_BENEFITS, BOARDS, type ScrapeFormState } from './constants';
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

  const showAuthBadge = AUTH_BENEFITS.has(form.board);
  const showAuthHint = AUTH_BENEFITS.has(form.board) && !boardConnected;

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
                size="sm"
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
                placeholder={t('jobs.queryPlaceholder')}
                disabled={scraping}
                className="w-full bg-white/[0.03] text-sm text-foreground placeholder:text-foreground/25 disabled:opacity-50"
              />
            </div>

            {/* Board picker */}
            <div className="mb-4">
              <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55">
                {t('jobs.board')}
              </div>
              <div className="flex flex-wrap gap-1.5">
                {BOARDS.map(({ id, labelKey }) => {
                  const active = form.board === id;
                  return (
                    <Button
                      key={id}
                      variant="ghost"
                      disabled={scraping}
                      onClick={() => onFormChange({ board: id })}
                      className={cn(
                        'rounded-lg px-2.5 py-1 text-[11px] font-medium transition-all',
                        active
                          ? 'bg-brand/20 text-brand-soft ring-1 ring-brand/40'
                          : 'bg-white/[0.04] text-foreground/50 hover:bg-white/[0.07] hover:text-foreground/80',
                        'disabled:cursor-not-allowed disabled:opacity-40'
                      )}
                    >
                      {t(labelKey)}
                    </Button>
                  );
                })}
              </div>
            </div>

            <AuthModeBadge
              show={showAuthBadge}
              board={form.board}
              boardConnected={boardConnected}
              disconnectPending={disconnectPending}
              onDisconnect={onDisconnect}
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
                    className="h-full rounded-full bg-gradient-to-r from-brand to-primary"
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
                <Button size="sm" variant="ghost" onClick={onCancel}>
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
                size="sm"
                variant="glass"
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
