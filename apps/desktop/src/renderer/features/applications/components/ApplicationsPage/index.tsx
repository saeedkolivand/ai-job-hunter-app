import {
  ArrowDownWideNarrow,
  ChevronDown,
  ChevronRight,
  ClipboardList,
  Plus,
  Search,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { APPLICATION_STAGES } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Dropdown, EmptyState, ErrorState, Input, RowSkeleton, transition } from '@ajh/ui';

import { PageShell } from '@/components/layout/PageShell';
import { ApplicationRow } from '@/features/applications/components/ApplicationRow';
import {
  PipelineStrip,
  PipelineStripSkeleton,
} from '@/features/applications/components/PipelineStrip';
import { StatusNoteModal } from '@/features/applications/components/StatusNoteModal';
import { TrackJobModal } from '@/features/applications/components/TrackJobModal';
import {
  APPLICATION_SORTS,
  PIPELINE_GROUPS,
  sortApplications,
  stagesForGroup,
  toApplicationSort,
} from '@/features/applications/lib/pipeline';
import { Route } from '@/routes/applications.index';
import { useApplications, useSetApplicationStatus } from '@/services/use-applications';
import { useSessionStore } from '@/store/session-store';

/** How long the post-change "Add a note?" chip stays offered on a row. */
const NOTE_HINT_MS = 12_000;

export function ApplicationsPage() {
  const { t } = useTranslation();
  const { applications: appsSlice, setApplications, toggleApplicationSection } = useSessionStore();
  const { collapsedSections, filter, stageGroup } = appsSlice;
  const sort = toApplicationSort(appsSlice.sort);

  const [trackOpen, setTrackOpen] = useState(false);

  const { data: allApps = [], isLoading, isError } = useApplications();

  // Optional-note prompt, held PAGE-level and keyed by application id. It cannot
  // live on the row: the invalidation refetch that follows the status write moves
  // that row into a different (keyed) stage section, so React unmounts it — and
  // any prompt state with it — before the user can type.
  const [notePromptId, setNotePromptId] = useState<string | null>(null);
  const [noteError, setNoteError] = useState(false);
  // A stage change on the LIST does not open a dialog: moving several rows is a
  // batch action and a modal per change is a keystroke tax. The changed row grows
  // a transient "Add a note?" chip instead; ignoring it costs nothing. The detail
  // page keeps the immediate prompt — there the change IS the single task.
  const [noteHintId, setNoteHintId] = useState<string | null>(null);
  const noteStatus = useSetApplicationStatus();

  // Retire the chip after a while so an old row doesn't keep offering it — but
  // NEVER while it holds focus (WCAG 2.2.1): yanking the focused control out of
  // the DOM strands a keyboard user on <body>. Re-arm instead and re-check.
  useEffect(() => {
    if (!noteHintId) return;
    const timer = setInterval(() => {
      if (document.activeElement?.closest(`[data-note-chip="${noteHintId}"]`)) return;
      setNoteHintId(null);
    }, NOTE_HINT_MS);
    return () => clearInterval(timer);
  }, [noteHintId]);
  // Re-read from the CURRENT list, not from a value captured when the prompt
  // opened, so a stage change landing in between is never reverted by the note.
  const notePromptApp = notePromptId ? allApps.find((a) => a.id === notePromptId) : undefined;

  const handleSaveNote = (note: string) => {
    if (!notePromptApp) return;
    setNoteError(false);
    noteStatus.mutate(
      { id: notePromptApp.id, status: notePromptApp.status, note },
      {
        onSuccess: () => setNotePromptId(null),
        // Keep the dialog open on failure so the typed note is not discarded.
        onError: () => setNoteError(true),
      }
    );
  };

  // `?highlight=<applicationId>` deep-link (notification "View"). Seed a LOCAL
  // flash id from the param so the flash lives in component state, not the URL;
  // the param is cleared after consuming it (below) so a revisit doesn't re-flash.
  const { highlight } = Route.useSearch();
  const navigate = useNavigate();
  const [highlightId, setHighlightId] = useState<string | null>(null);

  // Consume the URL param once: seed the local flash + clear the param so a
  // refresh/revisit doesn't re-highlight.
  useEffect(() => {
    if (!highlight) return;
    setHighlightId(highlight);
    void navigate({ to: '/applications', search: {}, replace: true });
  }, [highlight, navigate]);

  // A deep-linked application (notification "View") must be visible, and the
  // highlight wins over BOTH things that can hide it: its stage section is
  // un-collapsed, and an active pipeline-strip filter that excludes it is
  // CLEARED outright. Clearing beats widening the filter — the user then sees
  // the same full list the notification implied, with no half-applied filter.
  useEffect(() => {
    if (!highlightId) return;
    const target = allApps.find((a) => a.id === highlightId);
    if (!target) return;
    const patch: { collapsedSections?: string[]; stageGroup?: string | null } = {};
    if (collapsedSections.includes(target.status)) {
      patch.collapsedSections = collapsedSections.filter((id) => id !== target.status);
    }
    const activeStages = stagesForGroup(stageGroup);
    if (activeStages && !activeStages.includes(target.status)) patch.stageGroup = null;
    if (Object.keys(patch).length > 0) setApplications(patch);
  }, [highlightId, allApps, collapsedSections, stageGroup, setApplications]);

  // Clear the local flash after ~3.5s so it fires once, not indefinitely.
  useEffect(() => {
    if (!highlightId) return;
    const timer = setTimeout(() => setHighlightId(null), 3500);
    return () => clearTimeout(timer);
  }, [highlightId]);

  // Text filter across company + title + candidate.
  const q = filter.trim().toLowerCase();
  const filtered = useMemo(
    () =>
      q
        ? allApps.filter(
            (a) =>
              a.company.toLowerCase().includes(q) ||
              a.title.toLowerCase().includes(q) ||
              a.candidate.toLowerCase().includes(q)
          )
        : allApps,
    [allApps, q]
  );

  // Pipeline-strip stage-group filter (null = every stage).
  const activeStages = stagesForGroup(stageGroup);

  // The per-row stage Tag is only informative when the group spans stages —
  // with a single-stage filter every row shares the one stage.
  const showStageTag = (activeStages?.length ?? 0) > 1;

  // Sections. ANY active stage filter (`activeStages !== null`) renders ONE FLAT
  // list: with a filter on, a section header is either a word-for-word copy of
  // the strip card already selected right above it (single-stage — same word,
  // different behaviour, and its collapse toggle is meaningless with one
  // section) or a second printing of each row's stage (aggregated — header plus
  // row Tag). Unfiltered, the stage sections are the whole navigation.
  const sections = useMemo(() => {
    if (activeStages) {
      const apps = sortApplications(
        filtered.filter((a) => activeStages.includes(a.status)),
        sort
      );
      return apps.length > 0 ? [{ stage: null, apps }] : [];
    }
    return APPLICATION_STAGES.map((stage) => ({
      stage,
      apps: sortApplications(
        filtered.filter((a) => a.status === stage.id),
        sort
      ),
    })).filter((s) => s.apps.length > 0);
  }, [filtered, activeStages, sort]);

  /** Clears BOTH filters — the escape hatch from an empty filtered result. */
  const showAll = () => setApplications({ stageGroup: null, filter: '' });

  // The active group's user-facing name, for the filtered-empty explanation.
  const activeGroupLabel =
    stageGroup === 'closed'
      ? t('applications.pipeline.closed')
      : PIPELINE_GROUPS.some((g) => g.id === stageGroup)
        ? t(`applications.stages.${stageGroup}` as const)
        : '';

  const actions = (
    <div className="flex items-center gap-2">
      <Input
        prefix={<Search size={12} />}
        value={filter}
        onChange={(e) => setApplications({ filter: e.target.value })}
        placeholder={t('applications.filterPlaceholder')}
        className="w-40 text-foreground/75 placeholder:text-foreground/30"
        variant="default"
        wrapperClassName="h-8"
        allowClear
      />
      {/* `role="group"` + aria-label rather than a <label for>: a <label> pointing
          at the Dropdown's <button> would REPLACE its accessible name (the current
          sort) instead of adding context to it. */}
      <div role="group" aria-label={t('applications.sort.label')} className="w-44">
        <Dropdown
          options={APPLICATION_SORTS.map((s) => ({
            value: s,
            label: t(`applications.sort.${s}` as const),
          }))}
          value={sort}
          onChange={(value) => setApplications({ sort: value })}
          icon={<ArrowDownWideNarrow size={12} />}
        />
      </div>
      <Button variant="primary" onClick={() => setTrackOpen(true)}>
        <Plus size={12} />
        {t('applications.trackButton')}
      </Button>
    </div>
  );

  return (
    <>
      <PageShell
        title={t('applications.title')}
        subtitle={t('applications.subtitle')}
        actions={actions}
        maxWidth="max-w-6xl 2xl:max-w-7xl"
      >
        {isLoading && (
          <div className="pt-4">
            {/* Reserves the strip's exact footprint (shared classes + real type
                sizes) so the rows below do not jump when the data lands. */}
            <PipelineStripSkeleton />
            <div className="space-y-2 pt-4">
              <RowSkeleton />
              <RowSkeleton />
              <RowSkeleton />
            </div>
          </div>
        )}

        {isError && (
          <ErrorState
            title={t('applications.errorTitle')}
            description={t('applications.errorDesc')}
            className="py-16"
          />
        )}

        {!isLoading && !isError && allApps.length === 0 && (
          <EmptyState
            icon={ClipboardList}
            title={t('applications.empty')}
            description={t('applications.emptyDesc')}
            action={
              <Button variant="primary" onClick={() => setTrackOpen(true)}>
                <Plus size={13} /> {t('applications.trackButton')}
              </Button>
            }
            className="py-16"
          />
        )}

        {!isLoading && !isError && allApps.length > 0 && (
          <div className="pt-4">
            <PipelineStrip
              applications={allApps}
              active={stageGroup}
              onSelect={(groupId) => setApplications({ stageGroup: groupId })}
              onShowOverdue={() => setApplications({ stageGroup: null, sort: 'nextAction' })}
            />
          </div>
        )}

        {!isLoading && !isError && allApps.length > 0 && sections.length === 0 && (
          <EmptyState
            icon={Search}
            title={t('applications.noResults')}
            // Name WHICH filters are hiding everything — an unexplained empty
            // list after a stage click reads as data loss.
            description={
              stageGroup && q
                ? t('applications.noResultsBoth', { stage: activeGroupLabel, query: filter.trim() })
                : stageGroup
                  ? t('applications.noResultsStage', { stage: activeGroupLabel })
                  : t('applications.noResultsQuery', { query: filter.trim() })
            }
            action={
              <Button variant="primary" onClick={showAll}>
                {t('applications.showAll')}
              </Button>
            }
            className="py-10"
          />
        )}

        <div className="space-y-4 pt-4">
          {sections.map(({ stage, apps }) => {
            const rows = (
              <div className="space-y-2">
                {apps.map((app) => (
                  <ApplicationRow
                    key={app.id}
                    application={app}
                    highlighted={app.id === highlightId}
                    showStageTag={showStageTag}
                    onStatusChanged={() => setNoteHintId(app.id)}
                    showNoteHint={app.id === noteHintId}
                    onAddNote={() => {
                      setNoteHintId(null);
                      setNoteError(false);
                      setNotePromptId(app.id);
                    }}
                  />
                ))}
              </div>
            );

            // Filtered view → ONE flat list; the selected strip card above already
            // names the scope, and a per-row Tag names the stage when it varies.
            if (!stage) return <div key="flat">{rows}</div>;

            const collapsed = collapsedSections.includes(stage.id);
            return (
              <div key={stage.id}>
                {/* Section header — collapsible; Button from @ajh/ui satisfies no-raw-button rule */}
                <Button
                  variant="unstyled"
                  onClick={() => toggleApplicationSection(stage.id)}
                  className="mb-2 flex w-full items-center gap-2 rounded-lg px-1 py-1 text-left text-xs font-semibold uppercase tracking-wider text-foreground/70 transition-colors hover:text-foreground/90"
                  aria-expanded={!collapsed}
                >
                  {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
                  {t(`applications.stages.${stage.id}` as const)}
                  <span className="ml-auto rounded-full bg-foreground/10 px-1.5 py-0.5 text-xs font-normal normal-case tracking-normal text-foreground/70">
                    {apps.length}
                  </span>
                </Button>

                <AnimatePresence initial={false}>
                  {!collapsed && (
                    <motion.div
                      key="section-content"
                      initial={{ opacity: 0, height: 0 }}
                      animate={{ opacity: 1, height: 'auto' }}
                      exit={{ opacity: 0, height: 0 }}
                      transition={transition.fast}
                      className="overflow-hidden"
                    >
                      {rows}
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            );
          })}
        </div>
      </PageShell>

      <StatusNoteModal
        open={notePromptApp !== undefined}
        onClose={() => setNotePromptId(null)}
        status={notePromptApp?.status ?? ''}
        company={notePromptApp?.company ?? ''}
        title={notePromptApp?.title ?? ''}
        isSaving={noteStatus.isPending}
        error={noteError ? t('applications.note.saveError') : null}
        onSave={handleSaveNote}
      />

      <TrackJobModal open={trackOpen} onClose={() => setTrackOpen(false)} />
    </>
  );
}
