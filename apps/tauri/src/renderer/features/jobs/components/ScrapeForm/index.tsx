import { Info, Loader2, Search, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';

import type { DATE_FILTER_OPTIONS } from '@ajh/shared';
import { Button, GlassCard, Input, LocationInput, SelectDropdown, cn, transition } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface ScrapeFormProps {
  show: boolean;
  form: {
    board: string;
    query: string;
    location: string;
    pages: number;
    dateFilter: '' | (typeof DATE_FILTER_OPTIONS)[number];
    locale: string;
  };
  scraping: boolean;
  scrapeOutcome: { ok: boolean; note?: string } | null;
  boardConnected: boolean;
  connectPending: boolean;
  disconnectPending: boolean;
  onToggle: () => void;
  onFormChange: (updates: Partial<ScrapeFormProps['form']>) => void;
  onStart: () => void;
  onCancel: () => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onGeocode: (query: string) => Promise<{ display: string }[]>;
}

const AUTH_BENEFITS = new Set(['linkedin', 'indeed', 'xing']);

const BOARDS = [
  { id: 'linkedin', labelKey: 'jobs.boards.linkedin' },
  { id: 'indeed', labelKey: 'jobs.boards.indeed' },
  { id: 'stepstone', labelKey: 'jobs.boards.stepstone' },
  { id: 'xing', labelKey: 'jobs.boards.xing' },
  { id: 'arbeitsagentur', labelKey: 'jobs.boards.arbeitsagentur' },
  { id: 'berlinstartupjobs', labelKey: 'jobs.boards.berlinstartupjobs' },
  { id: 'germantechjobs', labelKey: 'jobs.boards.germantechjobs' },
  { id: 'greenhouse', labelKey: 'jobs.boards.greenhouse' },
  { id: 'lever', labelKey: 'jobs.boards.lever' },
  { id: 'ashby', labelKey: 'jobs.boards.ashby' },
  { id: 'workday', labelKey: 'jobs.boards.workday' },
  { id: 'smartrecruiters', labelKey: 'jobs.boards.smartrecruiters' },
  { id: 'recruitee', labelKey: 'jobs.boards.recruitee' },
  { id: 'personio', labelKey: 'jobs.boards.personio' },
  { id: 'remoteok', labelKey: 'jobs.boards.remoteok' },
  { id: 'remotive', labelKey: 'jobs.boards.remotive' },
  { id: 'arbeitnow', labelKey: 'jobs.boards.arbeitnow' },
  { id: 'wwr', labelKey: 'jobs.boards.wwr' },
  { id: 'ycombinator', labelKey: 'jobs.boards.ycombinator' },
] as const;

const REGIONS = [
  { value: 'us', labelKey: 'jobs.regions.us' },
  { value: 'de', labelKey: 'jobs.regions.de' },
  { value: 'uk', labelKey: 'jobs.regions.uk' },
  { value: 'fr', labelKey: 'jobs.regions.fr' },
  { value: 'at', labelKey: 'jobs.regions.at' },
  { value: 'ch', labelKey: 'jobs.regions.ch' },
  { value: 'au', labelKey: 'jobs.regions.au' },
  { value: 'ca', labelKey: 'jobs.regions.ca' },
  { value: 'nl', labelKey: 'jobs.regions.nl' },
  { value: 'be', labelKey: 'jobs.regions.be' },
  { value: 'es', labelKey: 'jobs.regions.es' },
  { value: 'it', labelKey: 'jobs.regions.it' },
  { value: 'pl', labelKey: 'jobs.regions.pl' },
  { value: 'br', labelKey: 'jobs.regions.br' },
  { value: 'in', labelKey: 'jobs.regions.in' },
  { value: 'sg', labelKey: 'jobs.regions.sg' },
  { value: 'jp', labelKey: 'jobs.regions.jp' },
] as const;

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
          <GlassCard tone="graphite" highlight className="p-5">
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
              <div className="mb-2 text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/35">
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

            {/* Auth mode badge */}
            <AnimatePresence>
              {showAuthBadge && (
                <motion.div
                  key={`mode-${form.board}-${boardConnected ? 'auth' : 'guest'}`}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={transition.fast}
                  className="mb-3 flex items-center gap-1.5"
                >
                  {boardConnected ? (
                    <>
                      <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-medium text-emerald-400">
                        <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
                        {t('jobs.modeAuthenticated')}
                      </span>
                      <span className="text-[10px] text-foreground/35">
                        {t('jobs.modeAuthNote')}
                      </span>
                      <button
                        type="button"
                        disabled={disconnectPending}
                        onClick={onDisconnect}
                        className="ml-auto shrink-0 text-[10px] text-red-400/70 underline-offset-2 hover:text-red-400 hover:underline disabled:opacity-50"
                      >
                        {disconnectPending ? (
                          <Loader2 size={10} className="animate-spin" />
                        ) : (
                          t('jobs.disconnect')
                        )}
                      </button>
                    </>
                  ) : (
                    <>
                      <span className="inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-2 py-0.5 text-[10px] font-medium text-amber-400">
                        <span className="h-1.5 w-1.5 rounded-full bg-amber-400" />
                        {t('jobs.modeGuest')}
                      </span>
                      <span className="text-[10px] text-foreground/35">
                        {t('jobs.modeGuestNote')}
                      </span>
                    </>
                  )}
                </motion.div>
              )}
            </AnimatePresence>

            {/* Auth hint */}
            <AnimatePresence>
              {showAuthHint && (
                <motion.div
                  key="auth-hint"
                  initial={{ opacity: 0, height: 0, marginBottom: 0 }}
                  animate={{ opacity: 1, height: 'auto', marginBottom: 16 }}
                  exit={{ opacity: 0, height: 0, marginBottom: 0 }}
                  transition={transition.fast}
                  className="overflow-hidden"
                >
                  <div className="flex items-center gap-2 rounded-lg border border-blue-400/15 bg-blue-400/5 px-3 py-2 text-[11px] text-blue-200/75">
                    <Info size={12} className="shrink-0 text-blue-400/60" />
                    <span>{t('jobs.authHint')}</span>
                    <button
                      type="button"
                      disabled={connectPending}
                      onClick={onConnect}
                      className="ml-auto shrink-0 text-brand-soft underline-offset-2 hover:underline disabled:opacity-50"
                    >
                      {connectPending ? (
                        <Loader2 size={11} className="animate-spin" />
                      ) : (
                        t('jobs.authHintLink')
                      )}
                    </button>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {/* Filters row */}
            <div className="mb-4 grid grid-cols-4 gap-2">
              <div className="col-span-2">
                <label className="mb-1.5 block text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/35">
                  {t('jobs.location')}
                </label>
                <LocationInput
                  value={form.location}
                  onChange={(v) => onFormChange({ location: v })}
                  placeholder={t('jobs.locationPlaceholder')}
                  disabled={scraping}
                  onFetchSuggestions={onGeocode}
                />
              </div>
              <div>
                <label className="mb-1.5 block text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/35">
                  {t('jobs.posted')}
                </label>
                <SelectDropdown
                  options={[
                    { value: '', label: t('jobs.anyTime') },
                    ...(AUTH_BENEFITS.has(form.board) && boardConnected
                      ? [
                          { value: '30m', label: t('jobs.past30m') },
                          { value: '1h', label: t('jobs.past1h') },
                          { value: '2h', label: t('jobs.past2h') },
                          { value: '4h', label: t('jobs.past4h') },
                          { value: '8h', label: t('jobs.past8h') },
                        ]
                      : []),
                    { value: '24h', label: t('jobs.past24h') },
                    { value: 'week', label: t('jobs.pastWeek') },
                    { value: 'month', label: t('jobs.pastMonth') },
                  ]}
                  value={form.dateFilter}
                  onChange={(value) =>
                    onFormChange({ dateFilter: value as '' | (typeof DATE_FILTER_OPTIONS)[number] })
                  }
                  disabled={scraping}
                  placeholder={t('jobs.anyTime')}
                />
              </div>
              <div>
                <label className="mb-1.5 block text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/35">
                  {t('jobs.pages')}
                </label>
                <Input
                  type="number"
                  min="1"
                  max="20"
                  value={form.pages}
                  onChange={(e) => onFormChange({ pages: parseInt(e.target.value) || 1 })}
                  disabled={scraping}
                  className="w-full bg-white/[0.03] text-xs text-foreground disabled:opacity-50"
                />
              </div>

              {/* Indeed region */}
              {form.board === 'indeed' && (
                <div className="col-span-4">
                  <label className="mb-1.5 block text-[10px] font-medium uppercase tracking-[0.18em] text-foreground/35">
                    {t('jobs.region')}
                  </label>
                  <SelectDropdown
                    options={REGIONS.map((r) => ({ value: r.value, label: t(r.labelKey) }))}
                    value={form.locale}
                    onChange={(value) => onFormChange({ locale: value })}
                    disabled={scraping}
                    placeholder={t('jobs.selectRegion')}
                  />
                </div>
              )}
            </div>

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
                onClick={onStart}
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
