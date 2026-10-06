import { Info, Loader2, Search, X } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { flushSync } from 'react-dom';

import { AGGREGATOR_BOARD_ID, type BoardCatalogEntry, PROVIDER_SLOTS } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Button, type CompanyTypeaheadHandle, Input } from '@ajh/ui';

import { LocationFilterNote, WorkTypeFilterNote } from '@/components/scrape/LocationFilterNote';
import { SeededCompaniesNote } from '@/components/scrape/SeededCompaniesNote';
import { AUTH_BENEFITS } from '@/features/jobs/constants';
import { useHasProviderKey } from '@/services/use-ai-provider';
import { useBoardsCatalog, useBoardStatuses } from '@/services/use-boards';

import { BoardConnectChip } from './BoardConnectChip';
import { BoardPicker } from './BoardPicker';
import { CompanySlugField } from './CompanySlugField';
import type { ScrapeFormState } from './constants';
import { ScrapeFilters } from './ScrapeFilters';
import { ScrapeFooter } from './ScrapeFooter';

interface ScrapeFormProps {
  /** Mount gate. The drawer that hosts the form already unmounts it on close;
   *  this stays so the form can be rendered conditionally anywhere else. */
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
  // Lets the submit path flush a typed-but-unentered company slug deterministically
  // (blur-independent — WebKit doesn't reliably blur on a sibling-button click).
  const companyFieldRef = useRef<CompanyTypeaheadHandle>(null);

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

  // Boards that are selected and require login, filtered against catalog auth.
  const needsLoginBoards = listedBoards.filter(
    (e) => selectedSet.has(e.id) && (e.auth === 'optional' || e.auth === 'required')
  );

  // Selected boards + whether a location is set — drives the honest "location
  // filtered locally" picker hint for boards without server-side location support.
  const selectedListedBoards = listedBoards.filter((e) => selectedSet.has(e.id));
  const hasLocation = form.location.trim().length > 0;

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

  // Aggregator key hint — shown when the aggregator board is selected but the
  // Adzuna keys aren't configured. Derived from service hooks; no hardcoded values
  // beyond the board's stable catalog id ('aggregator').
  const aggregatorSelected = selectedSet.has(AGGREGATOR_BOARD_ID);
  const { data: adzunaIdData } = useHasProviderKey(PROVIDER_SLOTS.adzunaAppId, aggregatorSelected);
  const { data: adzunaKeyData } = useHasProviderKey(
    PROVIDER_SLOTS.adzunaAppKey,
    aggregatorSelected
  );
  const showAggregatorKeyHint = aggregatorSelected && !(adzunaIdData?.has && adzunaKeyData?.has);

  // Flush a typed-but-unentered company slug BEFORE starting, synchronously, so
  // `onStart` reads a `companies` array that already includes it. `flushSync`
  // forces the parent re-render before the scrape captures state (the guard the
  // old comma-input carried); blur-commit alone is not reliable on WebKit.
  const handleStart = () => {
    if (showCompanyInput) flushSync(() => companyFieldRef.current?.commitPending());
    onStart();
  };

  // Count label: "3 selected" — i18next picks the plural form automatically.
  const countLabel = t('jobs.boardsSelected', { count: form.boards.length });

  // Progress label
  const scrapingLabel =
    form.boards.length === 1
      ? (catalogRaw?.find((e) => e.id === form.boards[0])?.displayName ?? form.boards[0])
      : countLabel;

  if (!show) return null;

  return (
    // Pinned-chrome panel: header and footer are `shrink-0`, only the middle
    // scrolls. The drawer that hosts this is a full-height flex column, so the
    // Start button can never be pushed below the fold — which it was when the
    // whole form lived inside one scrolling body at the 900×600 floor.
    // No GlassCard and no entrance animation here: the drawer IS the surface and
    // owns the transition (a card-in-a-panel doubled both the inset and the fade).
    <div data-testid={TEST_IDS.jobs.scrapeForm} className="flex h-full min-h-0 flex-col">
      {/* Header — pinned */}
      <div className="flex shrink-0 items-center justify-between border-b border-[var(--border-clear)] px-5 py-3">
        <div className="flex items-center gap-2">
          <div className="flex h-5 w-5 items-center justify-center rounded-md bg-brand/15">
            <Search size={11} className="text-brand-soft" />
          </div>
          <h2 className="text-body-strong text-foreground/90">{t('jobs.newScrape')}</h2>
        </div>
        <Button
          variant="ghost"
          aria-label={t('common.close')}
          onClick={onToggle}
          className="rounded-md p-1 text-foreground/50 hover:bg-muted hover:text-foreground/80 h-auto"
        >
          <X size={13} />
        </Button>
      </div>

      {/* Body — the only scrolling region */}
      <div
        data-testid={TEST_IDS.jobs.scrapeFormScroll}
        className="min-h-0 flex-1 overflow-y-auto px-5 py-4"
      >
        {/* Query — hero input */}
        <div className="mb-4">
          <Input
            id="jobs-scrape-query"
            name="jobs-scrape-query"
            type="text"
            value={form.query}
            onChange={(e) => onFormChange({ query: e.target.value })}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !scraping && !blockedByRequiredLogin && form.query.trim()) {
                e.preventDefault();
                handleStart();
              }
            }}
            placeholder={t('jobs.queryPlaceholder')}
            disabled={scraping}
            allowClear
            className="w-full bg-field shadow-none text-sm text-foreground placeholder:text-foreground/25 disabled:opacity-50"
          />
        </div>

        <BoardPicker
          listedBoards={listedBoards}
          catalogLoading={catalogLoading}
          selected={form.boards}
          scraping={scraping}
          countLabel={countLabel}
          onFormChange={onFormChange}
        />

        {/* Auth affordance — compact "needs login" row per selected board */}
        {needsLoginBoards.length > 0 && (
          <div className="mb-3 flex flex-wrap items-center gap-1.5">
            <span className="text-[10px] text-foreground/55">{t('jobs.needsLogin.label')}</span>
            {needsLoginBoards.map((e) => (
              <BoardConnectChip key={e.id} board={e.id} required={e.auth === 'required'} />
            ))}
          </div>
        )}

        {/* Honest location hint — names selected boards that don't filter by
                location server-side (results are matched on-device instead). */}
        <LocationFilterNote boards={selectedListedBoards} hasLocation={hasLocation} />

        {/* Same honesty disclosure for work type — only smartrecruiters filters
                it server-side today; every other selected board's results are
                matched on-device instead. */}
        <WorkTypeFilterNote boards={selectedListedBoards} active={form.workTypes.length > 0} />

        {/* Seeded-companies disclosure — names the curated companies a
                company-scoped ATS board (Greenhouse/Lever/Ashby/…) will query (#621) */}
        <SeededCompaniesNote boards={selectedListedBoards} />

        {/* Aggregator key hint — shown when aggregator selected but Adzuna keys absent */}
        {showAggregatorKeyHint && (
          <p
            role="status"
            data-testid={TEST_IDS.jobs.aggregatorKeyHint}
            className="mb-3 flex items-center gap-1.5 text-[11px] text-amber-400/70"
          >
            <Info size={11} aria-hidden="true" />
            {t('jobs.aggregatorKeyHint')}
          </p>
        )}

        {/* Companies typeahead — only shown when an ATS board (requiresCompany)
                is selected. Slugs are added as chips feeding form.companies; the
                suggestions merge passively-harvested slugs (ADR-030) with the
                selected boards' curated seeds. */}
        {showCompanyInput && (
          <div className="mb-4">
            <label
              htmlFor="scrape-companies"
              className="mb-1.5 block text-[10px] font-semibold uppercase tracking-[0.18em] text-foreground/55"
            >
              {t('jobs.companies.label')}
            </label>
            <CompanySlugField
              ref={companyFieldRef}
              companies={form.companies}
              onChange={(companies) => onFormChange({ companies })}
              seededBoards={selectedListedBoards}
              disabled={scraping}
            />
          </div>
        )}

        <ScrapeFilters
          form={form}
          scraping={scraping}
          boardConnected={anyAuthBenefitConnected}
          onFormChange={onFormChange}
          onGeocode={onGeocode}
        />

        {/* Scraping status — an honest indeterminate signal only (the earlier
            fake 85% progress bar fabricated progress and was removed). The real
            per-board progress is the streamed-results count shown in JobsResults. */}
        {scraping && (
          <div className="flex items-center gap-1.5 text-[11px] text-foreground/60">
            <Loader2 size={10} className="animate-spin" />
            {t('jobs.scraping')} {scrapingLabel}…
          </div>
        )}
      </div>

      <ScrapeFooter
        scraping={scraping}
        scrapeOutcome={scrapeOutcome}
        queryEmpty={!form.query.trim()}
        unconnectedRequired={unconnectedRequired}
        onStart={handleStart}
        onCancel={onCancel}
      />
    </div>
  );
}
