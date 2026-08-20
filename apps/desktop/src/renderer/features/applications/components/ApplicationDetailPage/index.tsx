import {
  ArrowLeft,
  Banknote,
  CalendarClock,
  Check,
  ExternalLink,
  FileText,
  HelpCircle,
  type LucideIcon,
  Mail,
  MessageSquarePlus,
  MessagesSquare,
  StickyNote,
  Trash2,
  Undo2,
  UserPlus,
  UserRound,
  X,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import {
  type AiGenerationRecord,
  type Application,
  APPLICATION_STAGES,
  type AutopilotFoundJob,
  EVENT_SOURCE_EMAIL,
  EVENT_SOURCE_EMAIL_REJECT,
  type StatusEvent,
} from '@ajh/shared';
import { type TFunction, useTranslation } from '@ajh/translations';
import {
  ActionMenu,
  Button,
  CardSkeleton,
  cn,
  ConfirmModal,
  Dropdown,
  ErrorState,
  IconBadge,
  Input,
  JobDescription,
  RowSkeleton,
  SectionLabel,
  Tabs,
  Tag,
  TextArea,
  Timeline,
  transition,
  useNotification,
} from '@ajh/ui';

import { StatusNoteModal } from '@/features/applications/components/StatusNoteModal';
import { nextActionLabel } from '@/features/applications/lib/stale';
import { useSyncedBuffer } from '@/features/applications/lib/use-synced-buffer';
import {
  TailorFlow,
  type TailorFlowController,
  type TailorFlowPersistence,
} from '@/features/documents/components/TailorFlow';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { useDefaultResumeId } from '@/hooks/useDefaultResumeId';
import { DETAIL_TABS, type DetailTab, Route } from '@/routes/applications.$id';
import {
  useAcceptStatusEvent,
  useApplication,
  useDocuments,
  useDocumentText,
  useImportJobUrl,
  useOpenExternal,
  useRejectStatusEvent,
  useRemoveApplication,
  useResolveJobUrl,
  useSetApplicationStatus,
  useUpdateApplication,
} from '@/services';
import { useAiGenerations } from '@/services/use-ai-generations';
import { useSessionStore } from '@/store/session-store';

import { ApplyByEmailTab } from './ApplyByEmailTab';
import { InterviewPrepTab } from './InterviewPrepTab';

const STATUS_OPTIONS = APPLICATION_STAGES.map((s) => ({ value: s.id, label: s.id }));

/** http(s)-only guard — mirrors ApplicationRow's open-link security gate. */
const isHttpUrl = (url: string) => /^https?:\/\//i.test(url);

/** Format an epoch-ms timestamp for a `<input type="date">` value (YYYY-MM-DD, local). */
function toDateInputValue(ms?: number): string {
  if (!ms) return '';
  const d = new Date(ms);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

/** Parse a `<input type="date">` value to local start-of-day epoch ms, or null when empty. */
function fromDateInputValue(value: string): number | null {
  if (!value) return null;
  const parts = value.split('-').map(Number);
  const [y, m, d] = parts;
  if (!y || !m || !d) return null;
  return new Date(y, m - 1, d).getTime();
}

function formatEventDate(at: number): string {
  return new Date(at).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/** Map an application status to a Timeline dot colour (graceful substring match). */
function statusColor(status: string): 'red' | 'green' | 'blue' | 'brand' {
  const s = status.toLowerCase();
  if (/reject|declin|withdraw/.test(s)) return 'red';
  if (/offer|accept|hire/.test(s)) return 'green';
  if (/interview|screen/.test(s)) return 'blue';
  return 'brand';
}

/** An unconfirmed email-derived write — the only kind the timeline renders as
 *  provisional (Accept/Reject affordances, never presented as settled history). */
function isProvisionalEvent(e: StatusEvent): boolean {
  return e.source === EVENT_SOURCE_EMAIL && !e.confirmed;
}

/** The reversal row `rejectStatusEvent` appends when its compare-and-set wins —
 *  a correction in the trail, distinct from both a normal user transition and
 *  the provisional row it resolves. */
function isCorrectionEvent(e: StatusEvent): boolean {
  return e.source === EVENT_SOURCE_EMAIL_REJECT;
}

/** Provisional/correction rows read as unsettled — same muted grey as the
 *  Timeline's own pending-ghost node — rather than claiming a status colour. */
function timelineEventColor(e: StatusEvent): 'red' | 'green' | 'blue' | 'brand' | 'gray' {
  if (isProvisionalEvent(e) || isCorrectionEvent(e)) return 'gray';
  return statusColor(e.toStatus);
}

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

/** Slim header + bordered-panel chrome shared by loading / error / loaded states. */
function SlimLayout({
  onBack,
  backLabel,
  title,
  children,
}: {
  onBack: () => void;
  backLabel: string;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-3 border-b border-[var(--border-soft)] px-8 py-4">
        <Button
          onClick={onBack}
          variant="ghost"
          className="shrink-0 gap-1.5 text-foreground/50 hover:text-foreground/80"
        >
          <ArrowLeft size={14} /> {backLabel}
        </Button>
        <div className="min-w-0 flex-1">
          <span className="truncate text-base font-semibold text-foreground/90">{title}</span>
        </div>
      </div>
      <div className="min-h-0 flex-1 p-4">{children}</div>
    </div>
  );
}

/** The bordered tabbed-panel surface (fills its parent height). */
function PanelShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden rounded-lg border border-[var(--border-soft)] bg-card">
      {children}
    </div>
  );
}

interface LoadedProps {
  application: Application;
  events: StatusEvent[];
  onBack: () => void;
  backLabel: string;
  /** Ask the page (which outlives a refetch) to open the optional-note prompt. */
  onNotePrompt: (status: string, changed: boolean) => void;
}

function ApplicationDetailLoaded({
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
  const updateApplication = useUpdateApplication();
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

  // Save-on-blur editable buffers. Each re-seeds independently when ITS server
  // value changes (see `useSyncedBuffer`) — no remount, so focus and sibling
  // uncommitted text survive a write landing.
  const [notes, setNotes] = useSyncedBuffer(application.notes);
  const [contactName, setContactName] = useSyncedBuffer(application.contactName);
  const [contactEmail, setContactEmail] = useSyncedBuffer(application.contactEmail);
  const [comp, setComp] = useSyncedBuffer(application.comp);
  const [nextActionAt, setNextActionAt] = useSyncedBuffer(
    toDateInputValue(application.nextActionAt)
  );

  const stageOptions = STATUS_OPTIONS.map((o) => ({
    value: o.value,
    label: t(`applications.status.${o.value}` as const),
  }));

  const [statusError, setStatusError] = useState(false);
  // Surfaced when the backend rejects a contact write (e.g. a malformed email) —
  // mirrors ApplyByEmailTab, which edits the SAME canonical pair.
  const [contactNameError, setContactNameError] = useState(false);
  const [contactEmailError, setContactEmailError] = useState(false);

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
        <Button
          onClick={onBack}
          variant="ghost"
          className="shrink-0 gap-1.5 text-foreground/50 hover:text-foreground/80"
        >
          <ArrowLeft size={14} /> {backLabel}
        </Button>

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
                {nextState === 'overdue'
                  ? t('applications.detail.followUpOverdue', {
                      date: formatEventDate(application.nextActionAt),
                    })
                  : t('applications.detail.followUpDue', {
                      date: formatEventDate(application.nextActionAt),
                    })}
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
                  <div className="@container h-full overflow-y-auto px-6">
                    {/* Follow-up leads the sheet: the one field that drives the
                        backend reminder sweep, so it must not sit last. */}
                    <OverviewSection
                      icon={CalendarClock}
                      label={t('applications.detail.followUpSection')}
                    >
                      <div className="grid gap-4 @md:grid-cols-2">
                        <div className="flex flex-col gap-1.5">
                          <label
                            htmlFor="appdetail-next-action"
                            className="text-xs font-semibold text-foreground/70"
                          >
                            {t('applications.detail.nextActionLabel')}
                          </label>
                          <Input
                            id="appdetail-next-action"
                            variant="default"
                            type="date"
                            value={nextActionAt}
                            onChange={(e) => setNextActionAt(e.target.value)}
                            onBlur={() => {
                              const next = fromDateInputValue(nextActionAt);
                              if (next !== (application.nextActionAt ?? null)) {
                                updateApplication.mutate({
                                  id: application.id,
                                  nextActionAt: next,
                                });
                              }
                            }}
                            className="w-full"
                          />
                          <p
                            className={cn(
                              'text-fine-print',
                              nextState === 'overdue' ? 'text-red-400' : 'text-foreground/70'
                            )}
                          >
                            {nextState === 'overdue'
                              ? t('applications.detail.followUpOverdueHint')
                              : nextState === 'upcoming'
                                ? t('applications.detail.followUpUpcomingHint')
                                : t('applications.detail.followUpNoneHint')}
                          </p>
                        </div>
                      </div>
                    </OverviewSection>

                    <OverviewSection icon={StickyNote} label={t('applications.detail.notesLabel')}>
                      <label htmlFor="appdetail-notes" className="sr-only">
                        {t('applications.detail.notesLabel')}
                      </label>
                      <TextArea
                        id="appdetail-notes"
                        variant="glass"
                        rows={4}
                        className="!shadow-none"
                        placeholder={t('applications.detail.notesPlaceholder')}
                        value={notes}
                        onChange={(e) => setNotes(e.target.value)}
                        onBlur={() => {
                          if (notes !== application.notes) {
                            updateApplication.mutate({ id: application.id, notes });
                          }
                        }}
                      />
                    </OverviewSection>

                    <OverviewSection
                      icon={UserRound}
                      label={t('applications.detail.contactSection')}
                    >
                      <div className="grid gap-4 @md:grid-cols-2">
                        <div className="flex flex-col gap-1.5">
                          <label
                            htmlFor="appdetail-contact-name"
                            className="text-xs font-semibold text-foreground/70"
                          >
                            {t('applications.detail.contactNameLabel')}
                          </label>
                          <Input
                            id="appdetail-contact-name"
                            variant="default"
                            placeholder={t('applications.detail.contactNamePlaceholder')}
                            value={contactName}
                            onChange={(e) => {
                              setContactName(e.target.value);
                              setContactNameError(false);
                            }}
                            onBlur={() => {
                              if (contactName !== application.contactName) {
                                // A rejected write returns `{ error }` rather than
                                // throwing — surface it here exactly as
                                // ApplyByEmailTab does for the same canonical pair.
                                updateApplication.mutate(
                                  { id: application.id, contactName },
                                  {
                                    onSuccess: (data) => setContactNameError(!!data.error),
                                    onError: () => setContactNameError(true),
                                  }
                                );
                              }
                            }}
                          />
                          {contactNameError && (
                            <p className="text-fine-print text-red-400" role="alert">
                              {t('applications.detail.contactSaveError')}
                            </p>
                          )}
                        </div>

                        <div className="flex flex-col gap-1.5">
                          <label
                            htmlFor="appdetail-contact-email"
                            className="text-xs font-semibold text-foreground/70"
                          >
                            {t('applications.detail.contactEmailLabel')}
                          </label>
                          <Input
                            id="appdetail-contact-email"
                            variant="default"
                            type="email"
                            placeholder={t('applications.detail.contactEmailPlaceholder')}
                            value={contactEmail}
                            onChange={(e) => {
                              setContactEmail(e.target.value);
                              setContactEmailError(false);
                            }}
                            onBlur={() => {
                              if (contactEmail !== application.contactEmail) {
                                updateApplication.mutate(
                                  { id: application.id, contactEmail },
                                  {
                                    onSuccess: (data) => setContactEmailError(!!data.error),
                                    onError: () => setContactEmailError(true),
                                  }
                                );
                              }
                            }}
                          />
                          {contactEmailError && (
                            <p className="text-fine-print text-red-400" role="alert">
                              {t('applications.detail.email.emailInvalid')}
                            </p>
                          )}
                        </div>
                      </div>
                    </OverviewSection>

                    <OverviewSection
                      icon={Banknote}
                      label={t('applications.detail.compensationSection')}
                    >
                      <div className="grid gap-4 @md:grid-cols-2">
                        <div className="flex flex-col gap-1.5">
                          <label
                            htmlFor="appdetail-comp"
                            className="text-xs font-semibold text-foreground/70"
                          >
                            {t('applications.detail.compLabel')}
                          </label>
                          <Input
                            id="appdetail-comp"
                            variant="default"
                            placeholder={t('applications.detail.compPlaceholder')}
                            value={comp}
                            onChange={(e) => setComp(e.target.value)}
                            onBlur={() => {
                              if (comp !== application.comp) {
                                updateApplication.mutate({ id: application.id, comp });
                              }
                            }}
                          />
                        </div>
                      </div>
                    </OverviewSection>
                  </div>
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

/** Scroll + padding wrapper for the prose tabs (Timeline / Brief). */
function TabScroll({ children }: { children: React.ReactNode }) {
  return <div className="h-full space-y-4 overflow-y-auto px-6 py-5">{children}</div>;
}

interface TimelineEventBodyProps {
  event: StatusEvent;
  t: TFunction;
  statusLabel: (status: string) => string;
  onAccept: () => void;
  onReject: () => void;
  acceptPending: boolean;
  rejectPending: boolean;
}

/**
 * One Timeline row's content. Three renderings, driven entirely by
 * `event.source`/`event.confirmed` (never a fabricated confidence number —
 * nothing in the payload carries one):
 *  - provisional (unconfirmed email write): reads as a guess awaiting review
 *    ("we think this happened — is that right?"), with Accept/Reject.
 *  - correction (`email_reject`'s reversal row): reads as a correction in the
 *    trail, not a normal user transition.
 *  - everything else (user-sourced, or an accepted email write): today's
 *    plain transition row.
 */
function TimelineEventBody({
  event,
  t,
  statusLabel,
  onAccept,
  onReject,
  acceptPending,
  rejectPending,
}: TimelineEventBodyProps) {
  const provisional = isProvisionalEvent(event);
  const correction = isCorrectionEvent(event);

  const transitionText =
    event.fromStatus && event.fromStatus !== event.toStatus ? (
      <>
        <span className="text-foreground/55">{statusLabel(event.fromStatus)}</span>
        <span className="text-foreground/30">→</span>
        <span className="font-medium text-foreground/85">{statusLabel(event.toStatus)}</span>
      </>
    ) : (
      <span className="font-medium text-foreground/85">{statusLabel(event.toStatus)}</span>
    );

  // Descriptive, not a bare "Accept"/"Reject" repeated down the list — names
  // WHICH transition the action resolves, for the accessible name below.
  const transitionDesc =
    event.fromStatus && event.fromStatus !== event.toStatus
      ? t('applications.detail.timeline.transitionDesc', {
          from: statusLabel(event.fromStatus),
          to: statusLabel(event.toStatus),
        })
      : statusLabel(event.toStatus);

  return (
    <>
      {provisional && (
        <Tag color="warning" icon={<Mail size={9} />} className="mb-1 text-[9px]">
          {t('applications.detail.timeline.provisionalBadge')}
        </Tag>
      )}
      {correction && (
        <Tag color="default" icon={<Undo2 size={9} />} className="mb-1 text-[9px]">
          {t('applications.detail.timeline.correctionBadge')}
        </Tag>
      )}
      <span className="flex items-center gap-1.5">{transitionText}</span>
      {/* The backend writes a fixed, non-localized English literal into
          `note` for BOTH the auto-write itself ("email-derived
          (unconfirmed)") and its reversal ("reverted: email-derived status
          change rejected by the user") — never render either verbatim; the
          badge + localized hint below say the same thing translated. */}
      {event.note && !provisional && !correction && (
        <span className="mt-0.5 block text-[11px] text-foreground/55">{event.note}</span>
      )}
      {provisional && (
        <p className="mt-0.5 text-[11px] text-foreground/50">
          {t('applications.detail.timeline.provisionalHint')}
        </p>
      )}
      {correction && (
        <p className="mt-0.5 text-[11px] text-foreground/50">
          {t('applications.detail.timeline.correctionHint')}
        </p>
      )}
      {provisional && (
        <div className="mt-1.5 flex items-center gap-1.5">
          <Button
            variant="success"
            size="sm"
            loading={acceptPending}
            disabled={acceptPending || rejectPending}
            onClick={onAccept}
            aria-label={t('applications.detail.timeline.acceptAria', {
              transition: transitionDesc,
            })}
          >
            <Check size={11} />
            {t('applications.detail.timeline.accept')}
          </Button>
          <Button
            variant="danger"
            size="sm"
            loading={rejectPending}
            disabled={acceptPending || rejectPending}
            onClick={onReject}
            aria-label={t('applications.detail.timeline.rejectAria', {
              transition: transitionDesc,
            })}
          >
            <X size={11} />
            {t('applications.detail.timeline.reject')}
          </Button>
        </div>
      )}
    </>
  );
}

interface TimelineTabProps {
  application: Application;
  events: StatusEvent[];
  /** Ask the page (which outlives a refetch) to open the optional-note prompt. */
  onNotePrompt: (status: string, changed: boolean) => void;
}

/**
 * Timeline tab — its own component (like {@link BriefTab}/{@link DocumentsTab},
 * not inlined in `ApplicationDetailLoaded`) so `useNotification()` and the
 * accept/reject mutations are only reached once this tab actually mounts.
 * Every other tab — and every test that never visits Timeline — stays clear
 * of the "must be used within NotificationProvider" requirement those calls
 * carry.
 */
function TimelineTab({ application, events, onNotePrompt }: TimelineTabProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const formatRelative = useFormatRelativeTime(t, 'resumes.relativeTime');
  const acceptStatusEvent = useAcceptStatusEvent();
  const rejectStatusEvent = useRejectStatusEvent();

  // A provisional row's Accept/Reject buttons vanish once the mutation
  // resolves (the row re-renders as settled). Move focus to the stable
  // Timeline heading beforehand so it never falls back to `document.body`.
  const timelineHeadingRef = useRef<HTMLSpanElement>(null);

  // `acceptStatusEvent`/`rejectStatusEvent` are each ONE `useMutation()`
  // instance shared by every row, so `.variables`/`.isPending` reflect only
  // the MOST RECENT `mutate()` call — they cannot represent two concurrent
  // in-flight rows. Track in-flight eventIds ourselves instead: added here
  // (before the mutate call, so the pending row never has a gap), cleared in
  // `onSettled` (fires on success OR error, unlike `onSuccess`/`onError`
  // alone). Accept/reject get separate sets so the correct button shows its
  // own spinner even if a row somehow has both in flight.
  const [acceptingEventIds, setAcceptingEventIds] = useState<Set<number>>(() => new Set());
  const [rejectingEventIds, setRejectingEventIds] = useState<Set<number>>(() => new Set());

  // Both take the SPECIFIC row's `eventId` as a param — never a shared,
  // zero-arg closure. Two provisional rows can coexist (a confirmation email,
  // then a later rejection email, both still unreviewed); resolving "the
  // pending row" any other way let a click on one row's button act on a
  // DIFFERENT row entirely. See `StatusEvent.eventId`'s doc.
  const handleAcceptEvent = (eventId: number) => {
    setAcceptingEventIds((prev) => new Set(prev).add(eventId));
    acceptStatusEvent.mutate(
      { id: application.id, eventId },
      {
        // `applications_accept_status_event` returns `Value`, not `Result` —
        // a backend failure resolves as `{ error }` and `invoke` FULFILS, so
        // `onError` never fires for it. Check `data.error` here, same as the
        // contact-write handlers above, or a transient DB failure would
        // still show the success toast while the row stays provisional.
        onSuccess: (data) => {
          if (data.error) {
            notify.error({ message: t('applications.detail.timeline.acceptError') });
            return;
          }
          timelineHeadingRef.current?.focus();
          notify.success({ message: t('applications.detail.timeline.acceptSuccess') });
        },
        onError: () => notify.error({ message: t('applications.detail.timeline.acceptError') }),
        onSettled: () => {
          setAcceptingEventIds((prev) => {
            const next = new Set(prev);
            next.delete(eventId);
            return next;
          });
        },
      }
    );
  };
  const handleRejectEvent = (eventId: number) => {
    setRejectingEventIds((prev) => new Set(prev).add(eventId));
    rejectStatusEvent.mutate(
      { id: application.id, eventId },
      {
        // Same `{ error }`-on-resolve shape as accept above — check it before
        // ever showing the success toast.
        onSuccess: (data) => {
          if (data.error) {
            notify.error({ message: t('applications.detail.timeline.rejectError') });
            return;
          }
          timelineHeadingRef.current?.focus();
          // Deliberately NOT "reverted" — the compare-and-set may have lost
          // (the user changed the status by hand meanwhile), in which case
          // this only dismissed the provisional row. The rendered timeline
          // (a correction row iff the CAS won) is the source of truth.
          notify.success({ message: t('applications.detail.timeline.rejectSuccess') });
        },
        onError: () => notify.error({ message: t('applications.detail.timeline.rejectError') }),
        onSettled: () => {
          setRejectingEventIds((prev) => {
            const next = new Set(prev);
            next.delete(eventId);
            return next;
          });
        },
      }
    );
  };

  // `events()` orders by `at ASC, rowid ASC`; `Array#sort` is stable, so a
  // bare `b.at - a.at` keeps ASCENDING insertion order for any pair sharing
  // one `at` while every surrounding pair is descending. That's reachable
  // here: a reject appends its reversal row immediately after its
  // compare-and-set wins, so a correction and the provisional row it
  // resolves can share a millisecond. `eventId` IS the rowid — the same
  // descending direction as `at` keeps the backend's tie order intact.
  const orderedEvents = [...events].sort((a, b) => b.at - a.at || b.eventId - a.eventId);
  const statusLabel = (status: string) =>
    status ? t(`applications.status.${status}` as const) : t('applications.detail.created');

  return (
    <TabScroll>
      <div className="flex items-center justify-between gap-2">
        <span
          ref={timelineHeadingRef}
          tabIndex={-1}
          className="block rounded text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45 focus-visible:ring-2 focus-visible:ring-brand/50"
        >
          {t('applications.detail.timelineTitle')}
        </span>
        <Button
          variant="glass"
          size="sm"
          className="gap-1.5"
          onClick={() => onNotePrompt(application.status, false)}
        >
          <MessageSquarePlus size={12} />
          {t('applications.note.add')}
        </Button>
      </div>
      {orderedEvents.length === 0 ? (
        <p className="text-xs text-foreground/45">{t('applications.detail.timelineEmpty')}</p>
      ) : (
        <Timeline
          items={orderedEvents.map((e) => ({
            color: timelineEventColor(e),
            label: <span title={formatRelative(e.at)}>{formatEventDate(e.at)}</span>,
            children: (
              <TimelineEventBody
                event={e}
                t={t}
                statusLabel={statusLabel}
                // Per-row closures — each captures THIS row's `eventId`,
                // never a shared handler. See the comment on
                // `handleAcceptEvent`/`handleRejectEvent` above.
                onAccept={() => handleAcceptEvent(e.eventId)}
                onReject={() => handleRejectEvent(e.eventId)}
                // Own in-flight tracking, not `.isPending`/`.variables` on
                // the shared mutation hook — see the comment above
                // `acceptingEventIds`/`rejectingEventIds` for why a shared
                // observer can't represent two concurrent rows.
                acceptPending={acceptingEventIds.has(e.eventId)}
                rejectPending={rejectingEventIds.has(e.eventId)}
              />
            ),
          }))}
        />
      )}
    </TabScroll>
  );
}

/**
 * A flat Overview section on the white detail sheet: an {@link IconBadge} +
 * {@link SectionLabel} header over its fields, separated from the previous
 * section by a hairline (none above the first). Replaces the old nested cards.
 */
function OverviewSection({
  icon,
  label,
  children,
}: {
  icon: LucideIcon;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-3 border-t border-[var(--border-soft)] py-5 first:border-t-0">
      <div className="flex items-center gap-2">
        <IconBadge icon={icon} size="sm" />
        <SectionLabel>{label}</SectionLabel>
      </div>
      {children}
    </section>
  );
}

/** Brief & answers tab — company brief as prose + the answers list. */
function BriefTab({ application }: { application: Application }) {
  const { t } = useTranslation();
  const hasBrief = application.brief.trim().length > 0;
  const hasAnswers = application.answers.length > 0;
  const [editingJd, setEditingJd] = useState(false);
  const [jdDraft, setJdDraft] = useState('');
  const { mutate: updateApp, isPending: isSaving } = useUpdateApplication();
  const { mutate: fetchJd, isPending: isFetching, isError: fetchFailed } = useImportJobUrl();

  // Resolve-on-open: mirrors InterviewPrepTab — auto-fetch from URL when the
  // saved jobDescription is empty, so the tab is useful without a manual fetch.
  const initialDesc = application.jobDescription.trim();
  const shouldAutoResolve = !initialDesc;
  const resolved = useResolveJobUrl(application.jobUrl, shouldAutoResolve);
  const jdLoading = shouldAutoResolve && resolved.isFetching;
  const jobDesc = initialDesc || (resolved.data?.description ?? '').trim();
  const hasJd = jobDesc.length > 0;

  const startEdit = () => {
    // Seed from the displayed/resolved content so auto-resolved JD isn't lost
    // when the user opens the editor before the description has been persisted.
    setJdDraft(jobDesc);
    setEditingJd(true);
  };
  const cancelEdit = () => setEditingJd(false);
  const saveJd = (text: string) => {
    updateApp(
      { id: application.id, jobDescription: text },
      { onSuccess: () => setEditingJd(false) }
    );
  };

  // No generic empty-state early-return: the JD section renders its own recovery
  // panel when empty (paste/fetch), which is exactly what a freshly-imported
  // partial stub — no brief, no answers, no JD — needs. That panel IS the empty
  // experience.
  return (
    <TabScroll>
      {hasBrief && (
        <div className="space-y-2">
          <span className="block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
            {t('applications.detail.briefTitle')}
          </span>
          <p className="select-text whitespace-pre-wrap text-[12px] leading-relaxed text-foreground/70">
            {application.brief}
          </p>
        </div>
      )}

      {/* Job description — markdown render; edit toggle when populated; recovery panel when empty */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className="block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
            {t('applications.detail.jdTitle')}
          </span>
          {hasJd && !editingJd && (
            <Button
              variant="ghost"
              size="sm"
              className="h-5 px-1.5 text-[10px]"
              onClick={startEdit}
            >
              {t('applications.detail.jdEdit')}
            </Button>
          )}
        </div>
        {jdLoading && (
          <div role="status" aria-busy="true" aria-label={t('jobs.loadingDescription')}>
            <RowSkeleton />
          </div>
        )}
        {!jdLoading && hasJd && !editingJd && (
          <JobDescription
            markdown={jobDesc}
            className="max-w-prose select-text space-y-4 text-caption text-foreground/80"
          />
        )}
        {!jdLoading && (editingJd || !hasJd) && (
          <div className="space-y-2">
            {!hasJd && (
              <p className="text-[11px] text-foreground/55">{t('jobUrlImport.notFound')}</p>
            )}
            <TextArea
              value={jdDraft}
              onChange={(e) => setJdDraft(e.target.value)}
              placeholder={t('applications.detail.jdPlaceholder')}
              className="min-h-[120px] text-[12px]"
            />
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                onClick={() => saveJd(jdDraft)}
                disabled={isSaving || jdDraft.trim().length === 0}
              >
                {t('applications.detail.jdSave')}
              </Button>
              {editingJd && (
                <Button variant="ghost" size="sm" onClick={cancelEdit}>
                  {t('applications.detail.jdCancel')}
                </Button>
              )}
              {!hasJd && application.jobUrl && (
                <Button
                  variant="glass"
                  size="sm"
                  disabled={isFetching}
                  onClick={() => {
                    fetchJd(application.jobUrl, {
                      onSuccess: (posting) => {
                        const desc = posting?.description ?? '';
                        if (desc.trim()) {
                          updateApp({ id: application.id, jobDescription: desc });
                        }
                      },
                    });
                  }}
                >
                  {isFetching ? '…' : t('applications.detail.jdFetch')}
                </Button>
              )}
            </div>
            {fetchFailed && (
              <p className="text-xs text-red-400" role="alert">
                {t('jobUrlImport.failed')}
              </p>
            )}
          </div>
        )}
      </div>

      {hasAnswers && (
        <div className="space-y-3">
          <span className="block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
            {t('applications.detail.answersTitle')}
          </span>
          {application.answers.map((qa) => (
            <div key={qa.id}>
              <p className="text-[11px] font-medium text-foreground/70">{qa.question}</p>
              <p className="mt-0.5 whitespace-pre-wrap text-[11px] leading-relaxed text-foreground/55">
                {qa.answer}
              </p>
            </div>
          ))}
        </div>
      )}
    </TabScroll>
  );
}

interface DocumentsTabProps {
  application: Application;
  matchingGenerations: AiGenerationRecord[];
}

/**
 * Documents tab — a full-height host for the shared {@link TailorFlow} generator
 * seeded with the user's default résumé, mirroring the autopilot apply flow.
 * Wizard / template / ATS persistence lives on the `applicationApply` session
 * slice (this surface owns it); TailorFlow surfaces a controller so the toolbar
 * can drive its Questions / Referral modals.
 */
function DocumentsTab({ application, matchingGenerations }: DocumentsTabProps) {
  const { t } = useTranslation();
  const applicationApply = useSessionStore((s) => s.applicationApply);
  const setApplicationApply = useSessionStore((s) => s.setApplicationApply);
  const [controller, setController] = useState<TailorFlowController | null>(null);
  const updateApplication = useUpdateApplication();

  // Debounce-persist job-ad edits from TailorFlow back to application.jobDescription
  // so the Interview prep tab (and BriefTab) can read the updated text without
  // navigating away and back. 600ms debounce avoids a mutation per keystroke.
  // Refs keep the unmount flush free of stale-closure issues (no dep on application/mutate).
  // The id is captured together with the text at schedule time so a reuse of this
  // component instance for a different application (before the timer fires) cannot
  // flush A's text onto B's id.
  const jdPersistTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingJd = useRef<{ id: string; text: string } | null>(null);
  const mutateRef = useRef(updateApplication.mutate);
  mutateRef.current = updateApplication.mutate;

  const flushJd = () => {
    if (jdPersistTimer.current !== null) {
      clearTimeout(jdPersistTimer.current);
      jdPersistTimer.current = null;
    }
    if (pendingJd.current !== null) {
      mutateRef.current({ id: pendingJd.current.id, jobDescription: pendingJd.current.text });
      pendingJd.current = null;
    }
  };

  const handleJobDescChange = (text: string) => {
    pendingJd.current = { id: application.id, text };
    if (jdPersistTimer.current !== null) clearTimeout(jdPersistTimer.current);
    jdPersistTimer.current = setTimeout(flushJd, 600);
  };

  // Flush any pending edit on unmount instead of discarding it — this prevents
  // the edit from being lost when the user switches tabs before the 600ms fires.
  // All state accessed here is via refs so the empty-dep array is correct: the
  // cleanup reads the live ref values at the time it runs, not stale captures.
  const flushJdRef = useRef(flushJd);
  flushJdRef.current = flushJd;
  useEffect(
    () => () => {
      flushJdRef.current();
    },
    []
  );

  // Seed the résumé text ONCE at mount — wait for BOTH the documents list (which
  // resolves `defaultResumeId`) and the default résumé text so the one-shot
  // wizard seed is present before TailorFlow mounts. `useDefaultResumeId` reads
  // `useDocuments` internally; while that list loads it returns `null`, so we
  // must gate on the list load too or TailorFlow seeds empty and locks it in.
  const docsQuery = useDocuments();
  const defaultResumeId = useDefaultResumeId();
  const resumeQuery = useDocumentText(defaultResumeId);

  if (docsQuery.isLoading || (!!defaultResumeId && resumeQuery.isLoading)) {
    return (
      <div className="h-full overflow-y-auto px-6 py-5">
        <CardSkeleton />
      </div>
    );
  }

  // Prefer the autopilot one-shot seed (deep-link from Apply), then the user's
  // default résumé, then the most recent matching generation.
  const seedResumeText =
    (applicationApply.applySeedResume ?? '') ||
    (resumeQuery.data ?? '') ||
    (matchingGenerations[0]?.resumeText ?? '');

  // Seed the id ONLY when the seeded text IS the default résumé's text — the
  // autopilot one-shot and a previous generation's output have no saved-document
  // backing, and an id that doesn't match the visible text is the exact drift
  // `useResumeInput`'s `selectDoc` contract exists to prevent.
  const seedResumeDocId =
    seedResumeText && seedResumeText === resumeQuery.data
      ? (defaultResumeId ?? undefined)
      : undefined;

  // Generation-store session key. Empty job URLs (`z.string().default('')`) would
  // collide for every URL-less application, bleeding one application's live
  // tailoring session into another — so key those by the stable application id.
  // Real URLs keep the `autopilot:` key so the live session is shared across the
  // autopilot apply surface and this detail tab.
  const contextId =
    application.jobUrl.trim() === '' ? `app:${application.id}` : `autopilot:${application.jobUrl}`;

  const job: AutopilotFoundJob = {
    title: application.title,
    company: application.company,
    url: application.jobUrl,
    location: undefined,
    description: application.jobDescription || undefined,
    foundAt: application.createdAt,
    salaryMin: application.salaryMin,
    salaryMax: application.salaryMax,
    salaryCurrency: application.salaryCurrency,
  };

  // Self-describing read: only trust `applyRun` when it was written FOR this
  // application. Evaluated at render time (not in an effect), so it's correct
  // on the very first render even when this tab mounts (default tab) before
  // the parent's applyForId-reset effect has had a chance to run — see
  // `ApplicationApplySlice.applyRun`'s doc comment for the full hazard.
  const applyRun =
    applicationApply.applyRun?.forId === application.id ? applicationApply.applyRun : null;

  const persistence: TailorFlowPersistence = {
    wizardStep: applicationApply.applyWizardStep,
    wizardForm: applicationApply.applyWizardForm,
    templateId: applicationApply.applyTemplateId,
    atsMode: applicationApply.applyAtsMode,
    accent: applicationApply.applyAccent,
    letterLayoutId: applicationApply.applyLetterLayoutId,
    runId: applyRun?.runId ?? null,
    runJobId: applyRun?.jobId ?? null,
    setWizardStep: (v) => setApplicationApply({ applyWizardStep: v }),
    setWizardForm: (v) => setApplicationApply({ applyWizardForm: v }),
    setTemplateId: (v) => setApplicationApply({ applyTemplateId: v }),
    setAtsMode: (v) => setApplicationApply({ applyAtsMode: v }),
    setAccent: (v) => setApplicationApply({ applyAccent: v }),
    setLetterLayoutId: (v) => setApplicationApply({ applyLetterLayoutId: v }),
    setRun: (ids) =>
      setApplicationApply({
        applyRun: ids ? { forId: application.id, runId: ids.runId, jobId: ids.jobId } : null,
      }),
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Toolbar — Questions (only on `done`) + Referral */}
      <div className="flex shrink-0 items-center justify-end gap-2 border-b border-[var(--border-soft)] px-8 py-3">
        {controller?.stage === 'done' && (
          <Button
            variant="glass"
            onClick={() => controller.openQuestions()}
            className="shrink-0 gap-1.5 text-brand-soft"
          >
            <HelpCircle size={13} /> {t('autopilot.apply.questions.title')}
            {controller.questionsCount > 0 && (
              <span className="rounded-full bg-brand/15 px-1.5 py-0.5 text-[9px] text-brand-soft">
                {controller.questionsCount}
              </span>
            )}
          </Button>
        )}
        <Button
          variant="glass"
          disabled={!controller}
          onClick={() => controller?.openInterviewQuestions()}
          className="shrink-0 gap-1.5 text-brand-soft"
        >
          <MessagesSquare size={13} /> {t('applications.detail.interview.title')}
          {controller && controller.interviewQuestionsCount > 0 && (
            <span className="rounded-full bg-brand/15 px-1.5 py-0.5 text-[9px] text-brand-soft">
              {controller.interviewQuestionsCount}
            </span>
          )}
        </Button>
        <Button
          variant="glass"
          disabled={!controller}
          onClick={() => controller?.openReferral()}
          className="shrink-0 gap-1.5 text-brand-soft"
        >
          <UserPlus size={13} /> {t('autopilot.referral.open')}
        </Button>
      </div>

      {/* Shared tailoring body — full-height, matching the autopilot apply flow */}
      <div className="min-h-0 flex-1">
        <TailorFlow
          job={job}
          resumeText={seedResumeText}
          resumeDocId={seedResumeDocId}
          board={application.board ?? ''}
          contextId={contextId}
          jobUrl={application.jobUrl}
          seedGeneration={matchingGenerations[0]}
          persistence={persistence}
          onController={setController}
          applicationId={application.id}
          initialSummary={application.jobSummary ?? undefined}
          onJobDescChange={handleJobDescChange}
        />
      </div>
    </div>
  );
}
