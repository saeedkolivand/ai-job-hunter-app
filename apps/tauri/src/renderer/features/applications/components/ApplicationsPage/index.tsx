import { ChevronDown, ChevronRight, ClipboardList, Plus, Search } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { APPLICATION_STAGES } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, EmptyState, ErrorState, Input, RowSkeleton, transition } from '@ajh/ui';

import { PageShell } from '@/components/layout/PageShell';
import { ApplicationRow } from '@/features/applications/components/ApplicationRow';
import { TrackJobModal } from '@/features/applications/components/TrackJobModal';
import { Route } from '@/routes/applications';
import { useApplications } from '@/services/use-applications';
import { useSessionStore } from '@/store/session-store';

export function ApplicationsPage() {
  const { t } = useTranslation();
  const { applications: appsSlice, setApplications, toggleApplicationSection } = useSessionStore();
  const { collapsedSections, filter } = appsSlice;

  const [trackOpen, setTrackOpen] = useState(false);

  const { data: allApps = [], isLoading, isError } = useApplications();

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

  // A just-imported job (notification "View" deep-link) must be visible:
  // un-collapse its stage section so the highlighted row isn't hidden.
  useEffect(() => {
    if (!highlightId) return;
    const target = allApps.find((a) => a.id === highlightId);
    if (target && collapsedSections.includes(target.status)) {
      setApplications({
        collapsedSections: collapsedSections.filter((id) => id !== target.status),
      });
    }
  }, [highlightId, allApps, collapsedSections, setApplications]);

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

  // Group by stage in APPLICATION_STAGES order; hide stages with no applications.
  const sections = useMemo(
    () =>
      APPLICATION_STAGES.map((stage) => ({
        stage,
        apps: filtered.filter((a) => a.status === stage.id),
      })).filter((s) => s.apps.length > 0),
    [filtered]
  );

  const actions = (
    <div className="flex items-center gap-2">
      <Input
        prefix={<Search size={12} />}
        value={filter}
        onChange={(e) => setApplications({ filter: e.target.value })}
        placeholder={t('applications.filterPlaceholder')}
        className="w-40 text-xs text-foreground/75 placeholder:text-foreground/30"
        variant="default"
        wrapperClassName="h-7"
        allowClear
      />
      <Button size="sm" variant="glass" onClick={() => setTrackOpen(true)}>
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
      >
        {isLoading && (
          <div className="space-y-2 pt-4">
            <RowSkeleton />
            <RowSkeleton />
            <RowSkeleton />
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
              <Button variant="glass" size="sm" onClick={() => setTrackOpen(true)}>
                <Plus size={13} /> {t('applications.trackButton')}
              </Button>
            }
            className="py-16"
          />
        )}

        {!isLoading && !isError && allApps.length > 0 && sections.length === 0 && (
          <EmptyState icon={Search} title={t('applications.noResults')} className="py-10" />
        )}

        <div className="space-y-4 pt-4">
          {sections.map(({ stage, apps }) => {
            const collapsed = collapsedSections.includes(stage.id);
            return (
              <div key={stage.id}>
                {/* Section header — collapsible; Button from @ajh/ui satisfies no-raw-button rule */}
                <Button
                  variant="unstyled"
                  onClick={() => toggleApplicationSection(stage.id)}
                  className="mb-2 flex w-full items-center gap-2 rounded-lg px-1 py-1 text-left text-xs font-semibold uppercase tracking-wider text-foreground/40 transition-colors hover:text-foreground/60"
                  aria-expanded={!collapsed}
                >
                  {collapsed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
                  {t(`applications.stages.${stage.id}` as const)}
                  <span className="ml-auto rounded-full bg-foreground/10 px-1.5 py-0.5 text-[10px] font-normal normal-case tracking-normal text-foreground/50">
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
                      <div className="space-y-2">
                        {apps.map((app) => (
                          <ApplicationRow
                            key={app.id}
                            application={app}
                            highlighted={app.id === highlightId}
                          />
                        ))}
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            );
          })}
        </div>
      </PageShell>

      <TrackJobModal open={trackOpen} onClose={() => setTrackOpen(false)} />
    </>
  );
}
