import { useState } from 'react';

import type { ApplicationTrackRequest } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, Input, ModalShell } from '@ajh/ui';

import { useTrackApplication } from '@/services/use-applications';

interface TrackJobModalProps {
  open: boolean;
  onClose: () => void;
}

export function TrackJobModal({ open, onClose }: TrackJobModalProps) {
  const { t } = useTranslation();
  const trackMutation = useTrackApplication();

  const [form, setForm] = useState<ApplicationTrackRequest>({
    jobUrl: '',
    company: '',
    title: '',
    candidate: '',
    board: '',
  });

  const handleClose = () => {
    setForm({ jobUrl: '', company: '', title: '', candidate: '', board: '' });
    onClose();
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const req: ApplicationTrackRequest = {};
    if (form.jobUrl?.trim()) req.jobUrl = form.jobUrl.trim();
    if (form.company?.trim()) req.company = form.company.trim();
    if (form.title?.trim()) req.title = form.title.trim();
    if (form.candidate?.trim()) req.candidate = form.candidate.trim();
    await trackMutation.mutateAsync(req);
    handleClose();
  };

  const titleId = 'track-job-modal-title';

  return (
    <ModalShell open={open} onClose={handleClose} maxWidth="max-w-md" ariaLabelledby={titleId}>
      <form onSubmit={(e) => void handleSubmit(e)}>
        <div className="border-b border-white/5 px-6 py-5">
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
              value={form.jobUrl ?? ''}
              onChange={(e) => setForm((f) => ({ ...f, jobUrl: e.target.value }))}
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
              value={form.company ?? ''}
              onChange={(e) => setForm((f) => ({ ...f, company: e.target.value }))}
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
              value={form.title ?? ''}
              onChange={(e) => setForm((f) => ({ ...f, title: e.target.value }))}
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
              value={form.candidate ?? ''}
              onChange={(e) => setForm((f) => ({ ...f, candidate: e.target.value }))}
              placeholder={t('applications.trackModal.candidatePlaceholder')}
            />
          </div>
        </div>

        <div className="flex items-center justify-end gap-2 border-t border-white/5 px-6 py-4">
          <Button type="button" variant="ghost" size="md" onClick={handleClose}>
            {t('applications.trackModal.cancel')}
          </Button>
          <Button type="submit" variant="glass" size="md" loading={trackMutation.isPending}>
            {t('applications.trackModal.submit')}
          </Button>
        </div>
      </form>
    </ModalShell>
  );
}
