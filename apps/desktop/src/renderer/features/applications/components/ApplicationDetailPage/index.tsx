import {
  ArrowLeft,
  CalendarClock,
  ExternalLink,
  FileText,
  HelpCircle,
  type LucideIcon,
  MessagesSquare,
  StickyNote,
  Trash2,
  UserPlus,
  UserRound,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import {
  type AiGenerationRecord,
  type Application,
  APPLICATION_STAGES,
  type AutopilotFoundJob,
  type StatusEvent,
} from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import {
  ActionMenu,
  Button,
  CardSkeleton,
  ConfirmModal,
  Dropdown,
  ErrorState,
  IconBadge,
  Input,
  JobDescription,
  RowSkeleton,
  SectionLabel,
  Tabs,
  TextArea,
  Timeline,
  transition,
} from '@ajh/ui';

import {
  TailorFlow,
  type TailorFlowController,
  type TailorFlowPersistence,
} from '@/features/documents/components/TailorFlow';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { useDefaultResumeId } from '@/hooks/useDefaultResumeId';
import { DETAIL_TABS, type DetailTab, Route } from '@/routes/applications.$id';
import {
  useApplication,
  useDocuments,
  useDocumentText,
  useImportJobUrl,
  useOpenExternal,
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

const BACK_TO = { jobs: '/jobs', autopilot: '/autopilot', applications: '/applications' } as const;

export function ApplicationDetailPage() {
  const { id } = Route.useParams();
  const { t } = useTranslation();
  const navigate = useNavigate();

  const { data, isLoading, isError } = useApplication(id);
  const application = data?.application ?? null;
  const events = data?.events ?? [];

  const { from } = Route.useSearch();
  const backTarget = from ? BACK_TO[from] : '/applications';
  const back = () => void navigate({ to: backTarget });
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

  // Key by id so navigating between two detail pages (same route pattern, new
  // param) remounts the loaded view and re-seeds the save-on-blur edit buffers
  // from the new application — TanStack Router reuses the instance otherwise.
  return (
    <ApplicationDetailLoaded
      key={id}
      application={application}
      events={events}
      onBack={back}
      backLabel={backLabel}
    />
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
}

function ApplicationDetailLoaded({ application, events, onBack, backLabel }: LoadedProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const formatRelative = useFormatRelativeTime(t, 'resumes.relativeTime');
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

  // Save-on-blur editable buffers, seeded once from the loaded application.
  const [notes, setNotes] = useState(application.notes);
  const [contactName, setContactName] = useState(application.contactName);
  const [contactEmail, setContactEmail] = useState(application.contactEmail);
  const [comp, setComp] = useState(application.comp);
  const [nextActionAt, setNextActionAt] = useState(toDateInputValue(application.nextActionAt));

  const stageOptions = STATUS_OPTIONS.map((o) => ({
    value: o.value,
    label: t(`applications.status.${o.value}` as const),
  }));

  const handleStatusChange = (status: string) => {
    void setStatus.mutateAsync({ id: application.id, status });
  };

  // Documents are display-joined to this application by the `applicationId` FK
  // (set on the generation at save time; legacy rows are backfilled at boot). A
  // raw-vs-normalized `jobUrl` string compare never matches for query-id boards
  // like Indeed — the Application stores the normalized url, the generation the raw
  // one — so the FK is the robust link.
  const matchingGenerations = (aiGenerations.data ?? []).filter(
    (g) => g.applicationId === application.id
  );

  const orderedEvents = [...events].sort((a, b) => b.at - a.at);

  const statusLabel = (status: string) =>
    status ? t(`applications.status.${status}` as const) : t('applications.detail.created');

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
          </div>
          {application.company && (
            <div className="truncate text-[11px] text-foreground/40">{application.company}</div>
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
                            className="text-xs font-medium text-foreground/70"
                          >
                            {t('applications.detail.contactNameLabel')}
                          </label>
                          <Input
                            id="appdetail-contact-name"
                            variant="default"
                            placeholder={t('applications.detail.contactNamePlaceholder')}
                            value={contactName}
                            onChange={(e) => setContactName(e.target.value)}
                            onBlur={() => {
                              if (contactName !== application.contactName) {
                                updateApplication.mutate({ id: application.id, contactName });
                              }
                            }}
                          />
                        </div>

                        <div className="flex flex-col gap-1.5">
                          <label
                            htmlFor="appdetail-contact-email"
                            className="text-xs font-medium text-foreground/70"
                          >
                            {t('applications.detail.contactEmailLabel')}
                          </label>
                          <Input
                            id="appdetail-contact-email"
                            variant="default"
                            type="email"
                            placeholder={t('applications.detail.contactEmailPlaceholder')}
                            value={contactEmail}
                            onChange={(e) => setContactEmail(e.target.value)}
                            onBlur={() => {
                              if (contactEmail !== application.contactEmail) {
                                updateApplication.mutate({ id: application.id, contactEmail });
                              }
                            }}
                          />
                        </div>
                      </div>
                    </OverviewSection>

                    <OverviewSection
                      icon={CalendarClock}
                      label={t('applications.detail.trackingSection')}
                    >
                      <div className="grid gap-4 @md:grid-cols-2">
                        <div className="flex flex-col gap-1.5">
                          <label
                            htmlFor="appdetail-comp"
                            className="text-xs font-medium text-foreground/70"
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

                        <div className="flex flex-col gap-1.5">
                          <label
                            htmlFor="appdetail-next-action"
                            className="text-xs font-medium text-foreground/70"
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
                        </div>
                      </div>
                    </OverviewSection>
                  </div>
                )}

                {tab === 'timeline' && (
                  <TabScroll>
                    <span className="block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
                      {t('applications.detail.timelineTitle')}
                    </span>
                    {orderedEvents.length === 0 ? (
                      <p className="text-xs text-foreground/45">
                        {t('applications.detail.timelineEmpty')}
                      </p>
                    ) : (
                      <Timeline
                        items={orderedEvents.map((e) => ({
                          color: statusColor(e.toStatus),
                          label: <span title={formatRelative(e.at)}>{formatEventDate(e.at)}</span>,
                          children: (
                            <>
                              <span className="flex items-center gap-1.5">
                                {e.fromStatus ? (
                                  <>
                                    <span className="text-foreground/55">
                                      {statusLabel(e.fromStatus)}
                                    </span>
                                    <span className="text-foreground/30">→</span>
                                    <span className="font-medium text-foreground/85">
                                      {statusLabel(e.toStatus)}
                                    </span>
                                  </>
                                ) : (
                                  <span className="font-medium text-foreground/85">
                                    {statusLabel(e.toStatus)}
                                  </span>
                                )}
                              </span>
                              {e.note && (
                                <span className="mt-0.5 block text-[11px] text-foreground/55">
                                  {e.note}
                                </span>
                              )}
                            </>
                          ),
                        }))}
                      />
                    )}
                  </TabScroll>
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
              <p className="text-[11px] text-destructive">{t('jobUrlImport.failed')}</p>
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

  const persistence: TailorFlowPersistence = {
    wizardStep: applicationApply.applyWizardStep,
    wizardForm: applicationApply.applyWizardForm,
    templateId: applicationApply.applyTemplateId,
    atsMode: applicationApply.applyAtsMode,
    accent: applicationApply.applyAccent,
    letterLayoutId: applicationApply.applyLetterLayoutId,
    setWizardStep: (v) => setApplicationApply({ applyWizardStep: v }),
    setWizardForm: (v) => setApplicationApply({ applyWizardForm: v }),
    setTemplateId: (v) => setApplicationApply({ applyTemplateId: v }),
    setAtsMode: (v) => setApplicationApply({ applyAtsMode: v }),
    setAccent: (v) => setApplicationApply({ applyAccent: v }),
    setLetterLayoutId: (v) => setApplicationApply({ applyLetterLayoutId: v }),
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
