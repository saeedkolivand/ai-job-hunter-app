import { CalendarClock, ExternalLink, FileText, Trash2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { type Application, APPLICATION_STAGES, type StatusEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { ActionMenu, Button, ConfirmModal, Dropdown, Tabs, Tag, transition } from '@ajh/ui';

import { nextActionLabel } from '@/features/applications/lib/stale';
import { DETAIL_TABS, type DetailTab, Route } from '@/routes/applications.$id';
import { useOpenExternal, useRemoveApplication, useSetApplicationStatus } from '@/services';
import { useAiGenerations } from '@/services/use-ai-generations';
import { useSessionStore } from '@/store/session-store';

import { ApplyByEmailTab } from './ApplyByEmailTab';
import { BriefTab } from './BriefTab';
import { formatEventDate, isHttpUrl } from './detail-format';
import { BackButton, PanelShell } from './DetailChrome';
import { DocumentsTab } from './DocumentsTab';
import { InterviewPrepTab } from './InterviewPrepTab';
import { OverviewTab } from './OverviewTab';
import { TimelineTab } from './TimelineTab';
import { useOverviewFields } from './useOverviewFields';

const STATUS_OPTIONS = APPLICATION_STAGES.map((s) => ({ value: s.id, label: s.id }));

interface LoadedProps {
  application: Application;
  events: StatusEvent[];
  onBack: () => void;
  backLabel: string;
  /** Ask the page (which outlives a refetch) to open the optional-note prompt. */
  onNotePrompt: (status: string, changed: boolean) => void;
}

export function ApplicationDetailLoaded({
  application,
  events,
  onBack,
  backLabel,
  onNotePrompt,
}: LoadedProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const applicationApply = useSessionStore((s) => s.applicationApply);
  const setApplicationApply = useSessionStore((s) => s.setApplicationApply);

  const setStatus = useSetApplicationStatus();
  const openExternal = useOpenExternal();
  const remove = useRemoveApplication();
  const aiGenerations = useAiGenerations();

  const tab: DetailTab = Route.useSearch().tab ?? DETAIL_TABS[0];
  const setTab = (next: DetailTab) =>
    void navigate({
      to: '/applications/$id',
      params: { id: application.id },
      // Preserve `from` (and any other search) so switching tabs keeps the
      // origin-aware Back target instead of dropping it to the default.
      search: (prev) => ({ ...prev, tab: next }),
      replace: true,
    });

  // Reset the in-progress wizard form when this surface switches to a different
  // application so one application's résumé text doesn't bleed into another.
  // Template / ATS stay sticky globals. The guard makes this idempotent: once
  // `applyForId` matches, the effect no-ops, so full deps don't loop.
  useEffect(() => {
    if (applicationApply.applyForId !== application.id) {
      setApplicationApply({
        applyForId: application.id,
        applyWizardStep: 0,
        applyWizardForm: null,
        // Drop any autopilot one-shot seed/badge left over from another application.
        applySeedResume: null,
        applyMatchLevel: null,
        // …and any staged-run reconnect target. Not load-bearing for
        // correctness (DocumentsTab reads `applyRun` gated on `forId`, so a
        // stale entry left here is simply ignored by ANY other application)
        // — this just keeps the store from accumulating one abandoned run
        // per application switch.
        applyRun: null,
      });
    }
  }, [application.id, applicationApply.applyForId, setApplicationApply]);

  // Delete (mirrors ApplicationRow): keepDocs decides which variant + payload.
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [keepDocs, setKeepDocs] = useState(true);
  const openDelete = (keep: boolean) => {
    setKeepDocs(keep);
    setDeleteOpen(true);
  };
  const confirmDelete = async () => {
    await remove.mutateAsync({ id: application.id, keepDocuments: keepDocs });
    setDeleteOpen(false);
    onBack();
  };

  const overviewFields = useOverviewFields(application);

  const stageOptions = STATUS_OPTIONS.map((o) => ({
    value: o.value,
    label: t(`applications.status.${o.value}` as const),
  }));

  const [statusError, setStatusError] = useState(false);

  // Success/error effects run on the mutation callbacks — the note prompt only
  // opens once the transition is actually persisted.
  const handleStatusChange = (status: string) => {
    // Dropdown.select fires onChange even when the current option is re-picked;
    // without this a no-op re-pick would append a status event and prompt for a
    // note about a transition that never happened.
    if (status === application.status) return;
    setStatusError(false);
    setStatus.mutate(
      { id: application.id, status },
      {
        onSuccess: () => onNotePrompt(status, true),
        onError: () => setStatusError(true),
      }
    );
  };

  const nextState = nextActionLabel(application.nextActionAt);

  // Documents are display-joined to this application by the `applicationId` FK
  // (set on the generation at save time; legacy rows are backfilled at boot). A
  // raw-vs-normalized `jobUrl` string compare never matches for query-id boards
  // like Indeed — the Application stores the normalized url, the generation the raw
  // one — so the FK is the robust link.
  const matchingGenerations = (aiGenerations.data ?? []).filter(
    (g) => g.applicationId === application.id
  );

  return (
    <div className="flex h-full flex-col">
      {/* Slim header (persists across all tabs) */}
      <div className="flex shrink-0 items-center gap-3 border-b border-[var(--border-soft)] px-8 py-4">
        <BackButton onBack={onBack} backLabel={backLabel} />

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <FileText size={14} className="shrink-0 text-brand-soft" />
            <span className="truncate text-base font-semibold text-foreground/90">
              {application.title || t('applications.row.noTitle')}
            </span>
            {application.board && (
              <span className="shrink-0 rounded-full border border-[var(--border-soft)] bg-foreground/[0.04] px-2 py-0.5 text-[9px] uppercase tracking-wider text-foreground/55">
                {application.board}
              </span>
            )}
            {applicationApply.applyMatchLevel && (
              <span className="shrink-0 rounded-full bg-brand/10 px-2 py-0.5 text-[10px] font-medium text-brand-soft">
                {t(`autopilot.wizard.filter.matchLevel.${applicationApply.applyMatchLevel}`)}{' '}
                {t('autopilot.apply.match')}
              </span>
            )}
            {/* Follow-up reminder, promoted out of the Overview tab so it is
                visible from every tab (and tinted when it has already passed). */}
            {nextState !== 'none' && application.nextActionAt && (
              <Tag
                color={nextState === 'overdue' ? 'error' : 'processing'}
                icon={<CalendarClock size={9} />}
                className="shrink-0 rounded-full px-2 py-0.5 text-[9px] uppercase tracking-wider"
              >
                {t(
                  nextState === 'overdue'
                    ? 'applications.detail.followUpOverdue'
                    : 'applications.detail.followUpDue',
                  { date: formatEventDate(application.nextActionAt) }
                )}
              </Tag>
            )}
          </div>
          {application.company && (
            <div className="truncate text-[11px] text-foreground/40">{application.company}</div>
          )}
          {statusError && (
            <p role="alert" className="text-fine-print text-red-400">
              {t('applications.row.statusError')}
            </p>
          )}
        </div>

        <div className="shrink-0">
          <Dropdown
            options={stageOptions}
            value={application.status}
            onChange={handleStatusChange}
            tone="primary"
          />
        </div>
        {isHttpUrl(application.jobUrl) && (
          <Button
            variant="glass"
            onClick={() => openExternal.mutate(application.jobUrl)}
            className="shrink-0 gap-1.5"
          >
            <ExternalLink size={13} /> {t('applications.detail.jobLink')}
          </Button>
        )}
        <ActionMenu
          label={t('applications.row.actions')}
          items={[
            {
              label: t('applications.row.deleteKeepDocs'),
              icon: <Trash2 size={14} />,
              onSelect: () => openDelete(true),
            },
            {
              label: t('applications.row.deleteAll'),
              icon: <Trash2 size={14} />,
              destructive: true,
              onSelect: () => openDelete(false),
            },
          ]}
        />
      </div>

      {/* Bordered tabbed panel */}
      <div className="min-h-0 flex-1 p-4">
        <PanelShell>
          <Tabs
            items={DETAIL_TABS.map((tb) => ({
              value: tb,
              label: t(`applications.detail.tabs.${tb}` as const),
              ariaControls: `appdetail-panel-${tb}`,
            }))}
            value={tab}
            onChange={setTab}
            ariaLabel={t('applications.detail.tabsLabel')}
            size="sm"
            idBase="appdetail-tab"
            className="shrink-0 px-3 py-2"
          />

          <div
            role="tabpanel"
            id={`appdetail-panel-${tab}`}
            aria-labelledby={`appdetail-tab-${tab}`}
            className="min-h-0 flex-1"
          >
            <AnimatePresence mode="wait">
              <motion.div
                key={tab}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={transition.fast}
                className="h-full"
              >
                {tab === 'overview' && (
                  <OverviewTab fields={overviewFields} nextState={nextState} />
                )}

                {tab === 'timeline' && (
                  <TimelineTab
                    application={application}
                    events={events}
                    onNotePrompt={onNotePrompt}
                  />
                )}

                {tab === 'brief' && <BriefTab application={application} />}

                {tab === 'documents' && (
                  <DocumentsTab
                    application={application}
                    matchingGenerations={matchingGenerations}
                  />
                )}

                {tab === 'email' && (
                  <ApplyByEmailTab
                    application={application}
                    matchingGenerations={matchingGenerations}
                  />
                )}

                {tab === 'interview' && (
                  <InterviewPrepTab
                    application={application}
                    matchingGenerations={matchingGenerations}
                  />
                )}
              </motion.div>
            </AnimatePresence>
          </div>
        </PanelShell>
      </div>

      <ConfirmModal
        open={deleteOpen}
        onClose={() => setDeleteOpen(false)}
        onConfirm={() => void confirmDelete()}
        title={keepDocs ? t('applications.delete.keepTitle') : t('applications.delete.allTitle')}
        description={
          keepDocs ? t('applications.delete.keepDesc') : t('applications.delete.allDesc')
        }
        confirmText={t('applications.delete.confirm')}
        variant="danger"
        isConfirming={remove.isPending}
      />
    </div>
  );
}
