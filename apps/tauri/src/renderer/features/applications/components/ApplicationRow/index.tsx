import { Clock, ExternalLink, Trash2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { type Application, APPLICATION_STAGES } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { ActionMenu, cn, ConfirmModal, Dropdown, Tag } from '@ajh/ui';

import { isStale, nextActionLabel, staleDays } from '@/features/applications/lib/stale';
import { useOpenExternal, useRemoveApplication, useSetApplicationStatus } from '@/services';

interface ApplicationRowProps {
  application: Application;
  /** Flash + scroll this row into view once (e.g. a just-imported job). */
  highlighted?: boolean;
}

const STATUS_OPTIONS = APPLICATION_STAGES.map((s) => ({ value: s.id, label: s.id }));

// Tiny status-pill shape for the in-row display Tags. Plain Tags render a <span>
// with no onClick, so clicks bubble to the row and never block navigation.
const STATUS_TAG = 'rounded-full px-1.5 py-0.5 text-[9px] uppercase tracking-wider';

export function ApplicationRow({ application, highlighted = false }: ApplicationRowProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const setStatus = useSetApplicationStatus();
  const remove = useRemoveApplication();
  const openExternal = useOpenExternal();

  const [deleteOpen, setDeleteOpen] = useState(false);
  const [keepDocs, setKeepDocs] = useState(true);

  // Bring a just-imported row into view when it becomes the highlight target.
  const rowRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (highlighted) rowRef.current?.scrollIntoView({ behavior: 'smooth', block: 'center' });
  }, [highlighted]);

  const stale = isStale(application.updatedAt);
  const nextState = nextActionLabel(application.nextActionAt);
  const days = staleDays(application.updatedAt);

  const handleStatusChange = (status: string) => {
    void setStatus.mutateAsync({ id: application.id, status });
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
    void navigate({ to: '/applications/$id', params: { id: application.id } });
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
          <div className="flex items-center gap-2">
            <span className="truncate text-sm font-semibold text-foreground/95">
              {application.title || t('applications.row.noTitle')}
            </span>
            {stale && (
              <Tag color="warning" icon={<Clock size={8} />} className={STATUS_TAG}>
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
          </div>
          <div className="mt-0.5 text-xs text-foreground/50">{application.company}</div>
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
