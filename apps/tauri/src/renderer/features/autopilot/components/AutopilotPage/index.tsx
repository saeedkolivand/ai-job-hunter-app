import { AlertCircle, Plus, X, Zap } from 'lucide-react';
import { AnimatePresence } from 'motion/react';
import { useEffect } from 'react';
import { useNavigate } from '@tanstack/react-router';

import type { Autopilot, AutopilotFoundJob } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, CardSkeleton } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { AutopilotCard } from '@/features/autopilot/components/AutopilotCard';
import { CreationWizard } from '@/features/autopilot/components/CreationWizard';
import { EmptyState } from '@/features/autopilot/components/EmptyState';
import { useAutopilotRun } from '@/features/autopilot/hooks/useAutopilotRun';
import { scoreToLevel } from '@/features/autopilot/lib/match-level';
import { autopilotToWizardState } from '@/features/autopilot/lib/wizard-state';
import { Route } from '@/routes/autopilot.index';
import { useAutopilots, useInvalidateAutopilots, useSaveFromPosting } from '@/services';
import { useSessionStore } from '@/store/session-store';

function AutopilotPage() {
  const { t } = useTranslation();
  const { data: autopilotList = [], isLoading: loading } = useAutopilots();
  const autopilots = autopilotList;
  const { autopilot, setAutopilot, resetAutopilotWizard, setApplicationApply } = useSessionStore();
  const { creating, focusedId } = autopilot;
  const setCreating = (v: boolean) => setAutopilot({ creating: v });
  const resetWizard = resetAutopilotWizard;

  // `?focus=<autopilotId>` deep-link (notification "View"): focus that autopilot
  // — same path the tray/deep-link `onFocus` takes (`focusedId` flags the card to
  // auto-expand). Consume once, refresh the list, then clear the param so a
  // refresh/re-render doesn't re-trigger.
  const { focus } = Route.useSearch();
  const navigate = useNavigate();
  const invalidateAutopilots = useInvalidateAutopilots();
  useEffect(() => {
    if (!focus) return;
    setAutopilot({ focusedId: focus });
    invalidateAutopilots();
    void navigate({ to: '/autopilot', search: {}, replace: true });
  }, [focus, navigate, invalidateAutopilots, setAutopilot]);

  // Returning from an Apply (Back): the user deep-linked into an application from a
  // found job. Re-focus that autopilot on this mount so its found-jobs list stays
  // expanded instead of collapsing. One-shot — promoted to `focusedId` (which the
  // card consumes + scrolls to) and cleared, so it only fires the first mount back.
  useEffect(() => {
    const appliedId = useSessionStore.getState().autopilot.lastAppliedId;
    if (appliedId) setAutopilot({ focusedId: appliedId, lastAppliedId: null });
  }, [setAutopilot]);

  const { runStates, stepLogs, error, setError, handleRun, handleTogglePause, handleDelete } =
    useAutopilotRun();
  const saveFromPosting = useSaveFromPosting();

  const handleCreated = (ap: Autopilot) => {
    // #44 — kick off the first run immediately after *creating* a new autopilot
    // (not when editing an existing one). Read editingId before resetWizard
    // clears it. handleRun surfaces a friendly inline error if no browser.
    const wasCreate = autopilot.editingId === null;
    resetWizard();
    if (wasCreate) void handleRun(ap._id);
  };

  const handleEdit = (ap: Autopilot) => {
    setAutopilot({
      creating: true,
      editingId: ap._id,
      wizardStep: 0,
      wizardForm: autopilotToWizardState(ap),
    });
  };

  // Apply creates (or reuses, deduped by jobUrl) the Application for this job,
  // seeds the autopilot base résumé for the Documents-tab wizard, then deep-links
  // into the application detail — the single place tailoring happens. `from=autopilot`
  // makes the detail's Back button return here.
  const handleApply = async (job: AutopilotFoundJob, ap: Autopilot) => {
    try {
      const res = await saveFromPosting.mutateAsync({
        jobUrl: job.url,
        board: ap.target.board,
        company: job.company,
        title: job.title,
      });
      if (!res?.id) {
        setError(res?.error ?? 'Failed to create the application');
        return;
      }
      setApplicationApply({
        applyForId: res.id,
        applySeedResume: ap.resumeText ?? null,
        applyMatchLevel: typeof job.score === 'number' ? scoreToLevel(job.score) : null,
        applyWizardStep: 0,
        applyWizardForm: null,
      });
      // Remember which autopilot we applied from so Back re-expands it (consumed
      // on the Autopilot page's next mount).
      setAutopilot({ lastAppliedId: ap._id });
      void navigate({
        to: '/applications/$id',
        params: { id: res.id },
        search: { tab: 'documents', from: 'autopilot' },
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to create the application');
    }
  };

  return (
    <PageTransition className="h-full overflow-hidden">
      <div className="flex h-full flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-8 py-5 shrink-0">
          <div className="flex items-center gap-3">
            <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-brand/15">
              <Zap size={15} className="text-brand-soft" />
            </div>
            <div>
              <h1 className="text-base font-semibold text-foreground/90">{t('autopilot.title')}</h1>
              <p className="text-[11px] text-foreground/40">{t('autopilot.subtitle')}</p>
            </div>
          </div>
          <Button
            variant="primary"
            onClick={() => setCreating(true)}
            className="transition-all duration-150 ease-out"
          >
            <Plus size={13} /> {t('autopilot.newAutopilot')}
          </Button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-8 py-6">
          {error && (
            <div className="mb-4 flex items-center gap-2 rounded-xl border border-red-400/20 bg-red-400/5 px-4 py-3 text-xs text-red-300/80">
              <AlertCircle size={12} /> {error}
              <Button
                onClick={() => setError(null)}
                className="ml-auto h-auto bg-transparent border-transparent p-0"
              >
                <X size={11} />
              </Button>
            </div>
          )}

          {loading ? (
            <div className="space-y-3">
              <CardSkeleton />
              <CardSkeleton />
            </div>
          ) : autopilots.length === 0 ? (
            <EmptyState onNew={() => setCreating(true)} />
          ) : (
            <div className="space-y-3">
              {autopilots.map((ap) => {
                const runState = runStates[ap._id] ?? 'idle';
                return (
                  <AutopilotCard
                    key={ap._id}
                    autopilot={ap}
                    runState={runState}
                    stepLogs={stepLogs[ap._id] ?? []}
                    focused={focusedId === ap._id}
                    onFocusHandled={() => setAutopilot({ focusedId: null })}
                    onRun={() => void handleRun(ap._id)}
                    onTogglePause={() => void handleTogglePause(ap)}
                    onEdit={() => handleEdit(ap)}
                    onDelete={() => void handleDelete(ap._id)}
                    onApply={(job) => void handleApply(job, ap)}
                  />
                );
              })}
            </div>
          )}
        </div>
      </div>

      {/* Creation wizard overlay */}
      <AnimatePresence>
        {creating && <CreationWizard onDone={handleCreated} onCancel={resetWizard} />}
      </AnimatePresence>
    </PageTransition>
  );
}

export { AutopilotPage };
