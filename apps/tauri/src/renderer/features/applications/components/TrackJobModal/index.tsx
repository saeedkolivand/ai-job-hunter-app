import { useEffect, useRef } from 'react';
import { useForm } from 'react-hook-form';

import type { ApplicationTrackRequest } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Input, ModalShell } from '@ajh/ui';

import { useTrackApplication } from '@/services/use-applications';

type TrackFormValues = { jobUrl: string; company: string; title: string; candidate: string };

interface TrackJobModalProps {
  open: boolean;
  onClose: () => void;
}

export function TrackJobModal({ open, onClose }: TrackJobModalProps) {
  const { t } = useTranslation();
  const trackMutation = useTrackApplication();

  const { register, handleSubmit, reset } = useForm<TrackFormValues>({
    defaultValues: { jobUrl: '', company: '', title: '', candidate: '' },
  });

  // Guard stale in-flight mutations from closing/resetting a freshly reopened modal.
  // Each time `open` transitions to true a new session token is issued; only the
  // submit that captured the CURRENT token may call handleClose.
  const sessionRef = useRef(0);
  useEffect(() => {
    if (open) sessionRef.current += 1;
  }, [open]);

  const handleClose = () => {
    reset();
    onClose();
  };

  const onSubmit = handleSubmit(async (values) => {
    const session = sessionRef.current;
    const req: ApplicationTrackRequest = {};
    if (values.jobUrl.trim()) req.jobUrl = values.jobUrl.trim();
    if (values.company.trim()) req.company = values.company.trim();
    if (values.title.trim()) req.title = values.title.trim();
    if (values.candidate.trim()) req.candidate = values.candidate.trim();
    await trackMutation.mutateAsync(req);
    if (sessionRef.current === session) handleClose();
  });

  const titleId = 'track-job-modal-title';

  return (
    <ModalShell open={open} onClose={handleClose} maxWidth="max-w-md" ariaLabelledby={titleId}>
      <form onSubmit={(e) => void onSubmit(e)}>
        <div className="border-b border-[var(--border-soft)] px-6 py-5">
          <h2 id={titleId} className="text-base font-medium text-foreground">
            {t('applications.trackModal.title')}
          </h2>
          <p className="mt-1 text-sm text-foreground/50">{t('applications.trackModal.subtitle')}</p>
        </div>

        <div className="space-y-4 px-6 py-5">
          <div>
            <label
              htmlFor="track-url"
              className="mb-1.5 block text-xs font-medium text-foreground/60"
            >
              {t('applications.trackModal.urlLabel')}
            </label>
            <Input
              id="track-url"
              type="url"
              className="w-full"
              {...register('jobUrl')}
              placeholder={t('applications.trackModal.urlPlaceholder')}
            />
          </div>
          <div>
            <label
              htmlFor="track-company"
              className="mb-1.5 block text-xs font-medium text-foreground/60"
            >
              {t('applications.trackModal.companyLabel')}
            </label>
            <Input
              id="track-company"
              className="w-full"
              {...register('company')}
              placeholder={t('applications.trackModal.companyPlaceholder')}
            />
          </div>
          <div>
            <label
              htmlFor="track-title"
              className="mb-1.5 block text-xs font-medium text-foreground/60"
            >
              {t('applications.trackModal.titleLabel')}
            </label>
            <Input
              id="track-title"
              className="w-full"
              {...register('title')}
              placeholder={t('applications.trackModal.titlePlaceholder')}
            />
          </div>
          <div>
            <label
              htmlFor="track-candidate"
              className="mb-1.5 block text-xs font-medium text-foreground/60"
            >
              {t('applications.trackModal.candidateLabel')}
            </label>
            <Input
              id="track-candidate"
              className="w-full"
              {...register('candidate')}
              placeholder={t('applications.trackModal.candidatePlaceholder')}
            />
          </div>
        </div>

        <div className="flex items-center justify-end gap-2 border-t border-[var(--border-soft)] px-6 py-4">
          <Button type="button" variant="ghost" size="md" onClick={handleClose}>
            {t('applications.trackModal.cancel')}
          </Button>
          <Button type="submit" variant="primary" size="md" loading={trackMutation.isPending}>
            {t('applications.trackModal.submit')}
          </Button>
        </div>
      </form>
    </ModalShell>
  );
}
