import { Clock, ExternalLink, Trash2 } from 'lucide-react';
import { useState } from 'react';

import { type Application, APPLICATION_STAGES } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { ActionMenu, cn, ConfirmModal, SelectDropdown } from '@ajh/ui';

import { isStale, nextActionLabel, staleDays } from '@/features/applications/lib/stale';
import { useOpenExternal, useRemoveApplication, useSetApplicationStatus } from '@/services';

interface ApplicationRowProps {
  application: Application;
}

const STATUS_OPTIONS = APPLICATION_STAGES.map((s) => ({ value: s.id, label: s.id }));

export function ApplicationRow({ application }: ApplicationRowProps) {
  const { t } = useTranslation();
  const setStatus = useSetApplicationStatus();
  const remove = useRemoveApplication();
  const openExternal = useOpenExternal();

  const [deleteOpen, setDeleteOpen] = useState(false);
  const [keepDocs, setKeepDocs] = useState(true);

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

  const stageOptions = STATUS_OPTIONS.map((o) => ({
    value: o.value,
    label: t(`applications.status.${o.value}` as const),
  }));

  return (
    <>
      <div className="surface-card flex items-center gap-4 rounded-xl px-4 py-3 transition-colors hover:bg-foreground/[0.03]">
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
              <span className="flex items-center gap-1 rounded-full border border-amber-400/20 bg-amber-400/5 px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-amber-300/80">
                <Clock size={8} />
                {t('applications.row.noReply', { days })}
              </span>
            )}
            {nextState !== 'none' && (
              <span
                className={cn(
                  'rounded-full border px-1.5 py-0.5 text-[9px] uppercase tracking-wider',
                  nextState === 'overdue'
                    ? 'border-red-400/20 bg-red-400/5 text-red-300/80'
                    : 'border-blue-400/20 bg-blue-400/5 text-blue-300/80'
                )}
              >
                {nextState === 'overdue'
                  ? t('applications.row.overdue')
                  : t('applications.row.followUp')}
              </span>
            )}
          </div>
          <div className="mt-0.5 text-xs text-foreground/50">{application.company}</div>
        </div>

        {/* Status dropdown */}
        <div
          className="shrink-0"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
          role="presentation"
        >
          <SelectDropdown
            options={stageOptions}
            value={application.status}
            onChange={handleStatusChange}
          />
        </div>

        {/* Actions */}
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
              onSelect: () => handleDelete(false),
            },
          ]}
        />
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
