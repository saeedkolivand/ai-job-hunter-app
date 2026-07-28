import { Clock, ExternalLink, Trash2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { type Application, APPLICATION_STAGES } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { ActionMenu, Button, cn, ConfirmModal, Dropdown, Tag } from '@ajh/ui';

import { formatSalaryRange } from '@/features/applications/lib/salary';
import { isStale, nextActionLabel, staleDays } from '@/features/applications/lib/stale';
import { useFormatRelativeTime } from '@/hooks/use-format-relative-time';
import { useOpenExternal, useRemoveApplication, useSetApplicationStatus } from '@/services';

interface ApplicationRowProps {
  application: Application;
  /** Flash + scroll this row into view once (e.g. a just-imported job). */
  highlighted?: boolean;
  /**
   * Show the row's own stage as a Tag. Set by the list when the active pipeline
   * group aggregates several stages (`Closed`), so an aggregated card never
   * hides whether a pursuit was accepted, rejected, ghosted or withdrawn.
   */
  showStageTag?: boolean;
  /**
   * Fired with the newly-persisted stage so the LIST can offer an optional note.
   * The prompt cannot live in this row: the invalidation refetch that follows the
   * write re-sorts/re-sections the list, unmounting this row (and any state on
   * it) before the user could type. The page owns the dialog instead.
   */
  onStatusChanged?: (status: string) => void;
  /** Show the transient "Add a note?" affordance after a just-persisted change. */
  showNoteHint?: boolean;
  /** Fired when that affordance is clicked — the page opens the note dialog. */
  onAddNote?: () => void;
}

const STATUS_OPTIONS = APPLICATION_STAGES.map((s) => ({ value: s.id, label: s.id }));

// Compact status-pill shape for the in-row display Tags. Plain Tags render a
// <span> with no onClick, so clicks bubble to the row and never block navigation.
// `shrink-0` keeps a tag from being crushed when the title row wraps.
const STATUS_TAG = 'shrink-0 rounded-full px-1.5 py-0.5 text-xs uppercase tracking-wider';

export function ApplicationRow({
  application,
  highlighted = false,
  showStageTag = false,
  onStatusChanged,
  showNoteHint = false,
  onAddNote,
}: ApplicationRowProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const setStatus = useSetApplicationStatus();
  const remove = useRemoveApplication();
  const openExternal = useOpenExternal();
  const formatRelative = useFormatRelativeTime(t, 'resumes.relativeTime');

  const [deleteOpen, setDeleteOpen] = useState(false);
  const [keepDocs, setKeepDocs] = useState(true);
  const [statusError, setStatusError] = useState(false);

  // Bring a just-imported row into view when it becomes the highlight target.
  const rowRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (highlighted) rowRef.current?.scrollIntoView({ behavior: 'smooth', block: 'center' });
  }, [highlighted]);

  const stale = isStale(application.updatedAt);
  const nextState = nextActionLabel(application.nextActionAt);
  const days = staleDays(application.updatedAt);

  // Applied date wins over updated: once a pursuit has left `saved`, "when did I
  // apply" is the question the row answers. Both carry the full local timestamp
  // in `title` so the relative label stays scannable without losing precision.
  const stampAt = application.appliedAt ?? application.updatedAt;
  const stampLabel = application.appliedAt
    ? t('applications.row.appliedAgo', { when: formatRelative(application.appliedAt) })
    : t('applications.row.updatedAgo', { when: formatRelative(application.updatedAt) });

  const board = application.board.trim();
  const salary = formatSalaryRange(
    application.salaryMin,
    application.salaryMax,
    application.salaryCurrency
  );

  // Success/error effects run on the mutation callbacks (never optimistically):
  // the note prompt only opens once the transition is actually persisted.
  const handleStatusChange = (status: string) => {
    // Dropdown.select fires onChange even when the current option is re-picked;
    // without this a no-op re-pick would append a status event and prompt for a
    // note about a transition that never happened.
    if (status === application.status) return;
    setStatusError(false);
    setStatus.mutate(
      { id: application.id, status },
      {
        onSuccess: () => onStatusChanged?.(status),
        onError: () => setStatusError(true),
      }
    );
  };

  const handleDelete = (keep: boolean) => {
    setKeepDocs(keep);
    setDeleteOpen(true);
  };

  const confirmDelete = () => {
    void remove.mutateAsync({ id: application.id, keepDocuments: keepDocs });
    setDeleteOpen(false);
  };

  const openDetail = () => {
    void navigate({
      to: '/applications/$id',
      params: { id: application.id },
      search: { from: 'applications' },
    });
  };

  const onRowKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      openDetail();
    }
  };

  const stageOptions = STATUS_OPTIONS.map((o) => ({
    value: o.value,
    label: t(`applications.status.${o.value}` as const),
  }));

  return (
    <>
      <div
        ref={rowRef}
        role="button"
        tabIndex={0}
        aria-label={t('applications.detail.openAria', {
          title: application.title || t('applications.row.noTitle'),
        })}
        onClick={openDetail}
        onKeyDown={onRowKey}
        className={cn(
          'surface-card flex cursor-pointer items-center gap-4 rounded-xl px-4 py-3 transition-colors hover:bg-foreground/[0.03]',
          highlighted && 'ring-2 ring-brand/60'
        )}
      >
        {/* Company avatar */}
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-brand/10 text-[10px] font-semibold uppercase tracking-wider text-brand-soft">
          {(application.company || '?').slice(0, 2)}
        </div>

        {/* Main info */}
        <div className="min-w-0 flex-1">
          {/* `flex-wrap` + `min-w-0`: at 900px (and at the large text scale) three
              tags beside the title squeezed it to 0px and overlapped the stage
              Dropdown. Tags now drop to a second line instead of pushing. */}
          <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
            <span
              className="min-w-0 truncate text-sm font-semibold text-foreground/95"
              title={application.title || t('applications.row.noTitle')}
            >
              {application.title || t('applications.row.noTitle')}
            </span>
            {showStageTag && (
              <Tag color="default" className={STATUS_TAG}>
                {t(`applications.status.${application.status}` as const)}
              </Tag>
            )}
            {stale && (
              <Tag color="warning" icon={<Clock size={10} />} className={STATUS_TAG}>
                {t('applications.row.noReply', { days })}
              </Tag>
            )}
            {nextState !== 'none' && (
              <Tag color={nextState === 'overdue' ? 'error' : 'processing'} className={STATUS_TAG}>
                {nextState === 'overdue'
                  ? t('applications.row.overdue')
                  : t('applications.row.followUp')}
              </Tag>
            )}
            {showNoteHint && (
              // Zero-keystroke follow-on to a stage change: no dialog is forced,
              // the affordance is transient and ignoring it costs nothing.
              <Button
                variant="unstyled"
                onClick={(e) => {
                  e.stopPropagation();
                  onAddNote?.();
                }}
                className={cn(
                  'shrink-0 rounded-full border border-brand/40 px-2 py-0.5',
                  'text-xs font-semibold text-brand-soft transition-shadow',
                  'hover:ring-1 hover:ring-inset hover:ring-brand/40'
                )}
              >
                {t('applications.row.addNoteHint')}
              </Button>
            )}
          </div>
          {/* Sizes stay on the 12px floor — the meta items are told apart by
              weight and caps, not by phantom sub-12px steps that all render at
              the same size anyway. */}
          <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-foreground/70">
            <span className="truncate">{application.company}</span>
            {board && (
              <span className="shrink-0 rounded-full border border-[var(--border-soft)] bg-foreground/[0.04] px-1.5 py-0.5 text-xs font-semibold uppercase leading-none tracking-wider text-foreground/70">
                {t(`jobs.boards.${board}`, { defaultValue: board })}
              </span>
            )}
            {salary && (
              <span className="shrink-0 text-xs font-semibold text-foreground/70">{salary}</span>
            )}
            <span
              className="shrink-0 text-xs text-foreground/70"
              title={new Date(stampAt).toLocaleString()}
            >
              {stampLabel}
            </span>
            {statusError && (
              <span role="alert" className="shrink-0 text-xs text-red-400">
                {t('applications.row.statusError')}
              </span>
            )}
          </div>
        </div>

        {/* Status dropdown */}
        <div
          className="shrink-0"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') e.stopPropagation();
          }}
          role="presentation"
        >
          <Dropdown
            options={stageOptions}
            value={application.status}
            onChange={handleStatusChange}
            tone="primary"
          />
        </div>

        {/* Actions — stop propagation so opening the menu / deleting doesn't navigate. */}
        <div
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') e.stopPropagation();
          }}
          role="presentation"
        >
          <ActionMenu
            label={t('applications.row.actions')}
            items={[
              ...(/^https?:\/\//i.test(application.jobUrl ?? '')
                ? [
                    {
                      label: t('applications.row.openUrl'),
                      icon: <ExternalLink size={14} />,
                      onSelect: () => openExternal.mutate(application.jobUrl),
                    },
                  ]
                : []),
              {
                label: t('applications.row.deleteKeepDocs'),
                icon: <Trash2 size={14} />,
                onSelect: () => handleDelete(true),
              },
              {
                label: t('applications.row.deleteAll'),
                icon: <Trash2 size={14} />,
                destructive: true,
                onSelect: () => handleDelete(false),
              },
            ]}
          />
        </div>
      </div>

      <ConfirmModal
        open={deleteOpen}
        onClose={() => setDeleteOpen(false)}
        onConfirm={confirmDelete}
        title={keepDocs ? t('applications.delete.keepTitle') : t('applications.delete.allTitle')}
        description={
          keepDocs ? t('applications.delete.keepDesc') : t('applications.delete.allDesc')
        }
        confirmText={t('applications.delete.confirm')}
        variant="danger"
        isConfirming={remove.isPending}
      />
    </>
  );
}
