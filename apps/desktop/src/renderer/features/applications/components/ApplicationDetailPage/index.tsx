import { useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { CardSkeleton, ErrorState, RowSkeleton } from '@ajh/ui';

import { StatusNoteModal } from '@/features/applications/components/StatusNoteModal';
import { Route } from '@/routes/applications.$id';
import { useApplication, useSetApplicationStatus } from '@/services';
import { useSessionStore } from '@/store/session-store';

import { ApplicationDetailLoaded } from './ApplicationDetailLoaded';
import { PanelShell, SlimLayout } from './DetailChrome';

const BACK_TO = { jobs: '/jobs', autopilot: '/autopilot', applications: '/applications' } as const;

export function ApplicationDetailPage() {
  const { id } = Route.useParams();
  const { t } = useTranslation();
  const navigate = useNavigate();

  const { data, isLoading, isError } = useApplication(id);
  const application = data?.application ?? null;
  const events = data?.events ?? [];

  // The optional-note prompt lives HERE, above `ApplicationDetailLoaded`, because
  // saving a status writes the record and the invalidation refetch re-renders the
  // loaded view — state held inside it does not reliably survive that churn (and
  // did not at all while the view was keyed by `updatedAt`). Declared before the
  // early returns so the hook order is stable across loading/error/loaded.
  const [noteFor, setNoteFor] = useState<string | null>(null);
  const [noteAfterChange, setNoteAfterChange] = useState(false);
  const [noteError, setNoteError] = useState(false);
  const noteStatus = useSetApplicationStatus();

  const openNotePrompt = (status: string, changed: boolean) => {
    setNoteError(false);
    setNoteAfterChange(changed);
    setNoteFor(status);
  };

  // Re-read the CURRENT status at save time rather than re-writing the stage
  // captured when the prompt opened: a transition landing in between (another
  // tab, the extension bridge) would otherwise be silently reverted by the note.
  const handleSaveNote = (note: string) => {
    if (!application) return;
    setNoteError(false);
    noteStatus.mutate(
      { id: application.id, status: application.status, note },
      {
        onSuccess: () => setNoteFor(null),
        // Keep the dialog open on failure so the typed note is not discarded.
        onError: () => setNoteError(true),
      }
    );
  };

  const noteModal = (
    <StatusNoteModal
      open={noteFor !== null}
      onClose={() => setNoteFor(null)}
      status={application?.status ?? noteFor ?? ''}
      company={application?.company ?? ''}
      title={application?.title ?? ''}
      changed={noteAfterChange}
      isSaving={noteStatus.isPending}
      error={noteError ? t('applications.note.saveError') : null}
      onSave={handleSaveNote}
    />
  );

  const { from } = Route.useSearch();
  const backTarget = from ? BACK_TO[from] : '/applications';
  // Gate on the ACTUAL compensating state, not just the `from` label: `from`
  // is a URL search param that survives native forward-navigation (mouse
  // forward / Alt+Right — nothing intercepts webview history), while
  // `lastAppliedId` is the one-shot session-store field AutopilotPage's focus
  // effect consumes on its NEXT mount. A second arrival at the same
  // ?from=autopilot URL with no pending focus left must fall back to the
  // router's default scroll reset — there's no compensating scroll to replace it.
  const hasPendingAutopilotFocus = useSessionStore((s) => s.autopilot.lastAppliedId !== null);
  // Returning to Autopilot from an Apply: that page's own focus effect
  // re-expands the source card and scrollIntoView's the applied job — the ONE
  // scroll motion this trip needs. Skip the router's own scroll reset/restore
  // for that hop only, or it fires first and the list visibly scrolls twice
  // (an old/reset position, then the focus jump). `from` alone isn't trusted
  // here — it's only meaningful alongside `hasPendingAutopilotFocus` above (a
  // backend/notification-driven `?from=autopilot` with no pending focus, e.g.
  // routes/__root.tsx or use-notifications.ts, correctly falls through to the
  // router's default reset since the state gate still requires it).
  const back = () =>
    void navigate({
      to: backTarget,
      resetScroll: !(from === 'autopilot' && hasPendingAutopilotFocus),
    });
  const backLabel =
    from === 'jobs'
      ? t('applications.detail.backJobs')
      : from === 'autopilot'
        ? t('applications.detail.backAutopilot')
        : t('applications.detail.back'); // default + 'applications' → "Back to applications"

  if (isLoading) {
    return (
      <SlimLayout onBack={back} backLabel={backLabel} title={t('applications.title')}>
        <PanelShell>
          <div className="h-full space-y-4 overflow-y-auto px-6 py-5">
            <RowSkeleton />
            <CardSkeleton />
            <CardSkeleton />
          </div>
        </PanelShell>
      </SlimLayout>
    );
  }

  if (isError || !application) {
    return (
      <SlimLayout onBack={back} backLabel={backLabel} title={t('applications.title')}>
        <PanelShell>
          <ErrorState
            title={t('applications.detail.notFound')}
            description={t('applications.detail.notFoundDesc')}
            className="py-16"
          />
        </PanelShell>
      </SlimLayout>
    );
  }

  // Key by id ONLY. Navigating between two detail pages (same route pattern, new
  // param) must remount — TanStack Router reuses the instance otherwise — but a
  // refetch of the SAME record must NOT: remounting on every persisted write
  // destroys keyboard focus, discards text typed into another field while the
  // first write is in flight, and tears down the whole TailorFlow sub-tree.
  // Re-seeding the save-on-blur buffers after an out-of-band write (the
  // apply-by-email tab shares the canonical contact pair) is handled per-field
  // inside `ApplicationDetailLoaded` instead — see `useSyncedBuffer`.
  return (
    <>
      <ApplicationDetailLoaded
        key={id}
        application={application}
        events={events}
        onBack={back}
        backLabel={backLabel}
        onNotePrompt={openNotePrompt}
      />
      {noteModal}
    </>
  );
}
