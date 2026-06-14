import { ArrowLeft, ExternalLink, FileText } from 'lucide-react';
import { useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { type Application, APPLICATION_STAGES, type StatusEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import {
  Button,
  CardSkeleton,
  Dropdown,
  EmptyState,
  ErrorState,
  GlassCard,
  Input,
  RowSkeleton,
  TextArea,
} from '@ajh/ui';

import { PageShell } from '@/components/layout/PageShell';
import { GenerationCard } from '@/features/documents/components/GenerationCard';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { Route } from '@/routes/applications.$id';
import {
  useApplication,
  useOpenExternal,
  useSetApplicationStatus,
  useUpdateApplication,
} from '@/services';
import { useAiGenerations } from '@/services/use-ai-generations';
import { useSessionStore } from '@/store/session-store';

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

export function ApplicationDetailPage() {
  const { id } = Route.useParams();
  const { t } = useTranslation();
  const navigate = useNavigate();

  const { data, isLoading, isError } = useApplication(id);
  const application = data?.application ?? null;
  const events = data?.events ?? [];

  const back = () => void navigate({ to: '/applications' });

  if (isLoading) {
    return (
      <PageShell title={t('applications.title')}>
        <div className="space-y-4 pt-4">
          <RowSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      </PageShell>
    );
  }

  if (isError || !application) {
    return (
      <PageShell
        title={t('applications.title')}
        actions={
          <Button variant="glass" onClick={back}>
            <ArrowLeft size={12} /> {t('applications.detail.back')}
          </Button>
        }
      >
        <ErrorState
          title={t('applications.detail.notFound')}
          description={t('applications.detail.notFoundDesc')}
          className="py-16"
        />
      </PageShell>
    );
  }

  // Key by id so navigating between two detail pages (same route pattern, new
  // param) remounts the loaded view and re-seeds the save-on-blur edit buffers
  // from the new application — TanStack Router reuses the instance otherwise.
  return (
    <ApplicationDetailLoaded key={id} application={application} events={events} onBack={back} />
  );
}

interface LoadedProps {
  application: Application;
  events: StatusEvent[];
  onBack: () => void;
}

function ApplicationDetailLoaded({ application, events, onBack }: LoadedProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const formatRelative = useFormatRelativeTime(t, 'resumes.relativeTime');
  const setAIGenerate = useSessionStore((s) => s.setAIGenerate);

  const setStatus = useSetApplicationStatus();
  const updateApplication = useUpdateApplication();
  const openExternal = useOpenExternal();
  const aiGenerations = useAiGenerations();

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

  // Documents are display-joined to this application by `jobUrl`. v1 uses a
  // trim-only comparison; normalization parity (case/scheme/query) is a follow-up.
  const appUrl = application.jobUrl.trim();
  const matchingGenerations = (aiGenerations.data ?? []).filter(
    (g) => appUrl !== '' && g.jobUrl.trim() === appUrl
  );

  const orderedEvents = [...events].sort((a, b) => b.at - a.at);

  const statusLabel = (status: string) =>
    status ? t(`applications.status.${status}` as const) : t('applications.detail.created');

  const goGenerate = () => {
    // Prefill is intentionally empty (v1) — the wizard resolves the rest.
    setAIGenerate({ jobAd: '', stage: 'idle', meta: null });
    void navigate({ to: '/ai-generate' });
  };

  const actions = (
    <div className="flex items-center gap-2">
      {isHttpUrl(application.jobUrl) && (
        <Button variant="glass" onClick={() => openExternal.mutate(application.jobUrl)}>
          <ExternalLink size={12} /> {t('applications.row.openUrl')}
        </Button>
      )}
      <Button variant="glass" onClick={onBack}>
        <ArrowLeft size={12} /> {t('applications.detail.back')}
      </Button>
    </div>
  );

  return (
    <PageShell
      title={application.title || t('applications.row.noTitle')}
      subtitle={application.company}
      actions={actions}
    >
      <div className="space-y-4 pt-4">
        {/* Status + timeline */}
        <GlassCard className="space-y-4 p-5">
          <div className="flex flex-wrap items-center gap-3">
            <span className="text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
              {t('applications.detail.statusTitle')}
            </span>
            <div className="shrink-0">
              <Dropdown
                options={stageOptions}
                value={application.status}
                onChange={handleStatusChange}
                tone="primary"
              />
            </div>
            {application.board && (
              <span className="rounded-full border border-white/[0.06] bg-white/[0.03] px-2 py-0.5 text-[9px] uppercase tracking-wider text-foreground/55">
                {application.board}
              </span>
            )}
          </div>

          <div>
            <span className="mb-2 block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
              {t('applications.detail.timelineTitle')}
            </span>
            {orderedEvents.length === 0 ? (
              <p className="text-xs text-foreground/45">{t('applications.detail.timelineEmpty')}</p>
            ) : (
              <ol className="space-y-2.5">
                {orderedEvents.map((e) => (
                  <li
                    key={`${e.at}-${e.toStatus}`}
                    className="flex flex-col gap-0.5 border-l border-white/[0.06] pl-3"
                  >
                    <span className="flex items-center gap-1.5 text-xs text-foreground/80">
                      {e.fromStatus ? (
                        <>
                          <span className="text-foreground/55">{statusLabel(e.fromStatus)}</span>
                          <span className="text-foreground/30">→</span>
                          <span className="font-medium">{statusLabel(e.toStatus)}</span>
                        </>
                      ) : (
                        <span className="font-medium">{statusLabel(e.toStatus)}</span>
                      )}
                    </span>
                    <span className="text-[10px] text-foreground/40" title={formatRelative(e.at)}>
                      {formatEventDate(e.at)}
                    </span>
                    {e.note && <span className="text-[11px] text-foreground/55">{e.note}</span>}
                  </li>
                ))}
              </ol>
            )}
          </div>
        </GlassCard>

        {/* Editable fields — save on blur (only when changed) */}
        <GlassCard className="space-y-4 p-5">
          <span className="block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
            {t('applications.detail.fieldsTitle')}
          </span>

          <label className="block space-y-1.5">
            <span className="text-xs text-foreground/60">
              {t('applications.detail.notesLabel')}
            </span>
            <TextArea
              variant="glass"
              rows={4}
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              onBlur={() => {
                if (notes !== application.notes) {
                  updateApplication.mutate({ id: application.id, notes });
                }
              }}
            />
          </label>

          <div className="grid gap-4 sm:grid-cols-2">
            <label className="block space-y-1.5">
              <span className="text-xs text-foreground/60">
                {t('applications.detail.contactNameLabel')}
              </span>
              <Input
                variant="default"
                value={contactName}
                onChange={(e) => setContactName(e.target.value)}
                onBlur={() => {
                  if (contactName !== application.contactName) {
                    updateApplication.mutate({ id: application.id, contactName });
                  }
                }}
              />
            </label>

            <label className="block space-y-1.5">
              <span className="text-xs text-foreground/60">
                {t('applications.detail.contactEmailLabel')}
              </span>
              <Input
                variant="default"
                type="email"
                value={contactEmail}
                onChange={(e) => setContactEmail(e.target.value)}
                onBlur={() => {
                  if (contactEmail !== application.contactEmail) {
                    updateApplication.mutate({ id: application.id, contactEmail });
                  }
                }}
              />
            </label>

            <label className="block space-y-1.5">
              <span className="text-xs text-foreground/60">
                {t('applications.detail.compLabel')}
              </span>
              <Input
                variant="default"
                value={comp}
                onChange={(e) => setComp(e.target.value)}
                onBlur={() => {
                  if (comp !== application.comp) {
                    updateApplication.mutate({ id: application.id, comp });
                  }
                }}
              />
            </label>

            <label className="block space-y-1.5">
              <span className="text-xs text-foreground/60">
                {t('applications.detail.nextActionLabel')}
              </span>
              <Input
                variant="default"
                type="date"
                value={nextActionAt}
                onChange={(e) => setNextActionAt(e.target.value)}
                onBlur={() => {
                  const next = fromDateInputValue(nextActionAt);
                  if (next !== (application.nextActionAt ?? null)) {
                    updateApplication.mutate({ id: application.id, nextActionAt: next });
                  }
                }}
                className="w-full"
              />
            </label>
          </div>
        </GlassCard>

        {/* Read-only: company brief */}
        {application.brief && (
          <GlassCard className="space-y-2 p-5">
            <span className="block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
              {t('applications.detail.briefTitle')}
            </span>
            <pre className="select-text whitespace-pre-wrap font-mono text-[11px] leading-relaxed text-foreground/55">
              {application.brief}
            </pre>
          </GlassCard>
        )}

        {/* Read-only: application answers */}
        {application.answers.length > 0 && (
          <GlassCard className="space-y-3 p-5">
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
          </GlassCard>
        )}

        {/* Documents */}
        <div className="space-y-3">
          <span className="block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
            {t('applications.detail.documentsTitle')}
          </span>
          {matchingGenerations.length > 0 ? (
            <div className="space-y-3">
              {matchingGenerations.map((g) => (
                <GenerationCard key={g.id} gen={g} />
              ))}
            </div>
          ) : (
            <EmptyState
              icon={FileText}
              title={t('applications.detail.noDocuments')}
              description={t('applications.detail.noDocumentsDesc')}
              action={
                <Button variant="primary" onClick={goGenerate}>
                  {t('applications.detail.generateDocs')}
                </Button>
              }
              className="py-12"
            />
          )}
        </div>
      </div>
    </PageShell>
  );
}
