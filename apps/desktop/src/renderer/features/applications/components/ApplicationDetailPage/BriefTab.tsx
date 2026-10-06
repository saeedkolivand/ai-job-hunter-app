import { useState } from 'react';

import type { Application } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, JobDescription, RowSkeleton, TextArea } from '@ajh/ui';

import { useImportJobUrl, useResolveJobUrl, useUpdateApplication } from '@/services';

import { TabScroll } from './DetailChrome';

const LABEL_CLASS =
  'block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45';

/** Brief & answers tab — company brief as prose + the answers list. */
export function BriefTab({ application }: { application: Application }) {
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
          <span className={LABEL_CLASS}>{t('applications.detail.briefTitle')}</span>
          <p className="select-text whitespace-pre-wrap text-[12px] leading-relaxed text-foreground/70">
            {application.brief}
          </p>
        </div>
      )}

      {/* Job description — markdown render; edit toggle when populated; recovery panel when empty */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className={LABEL_CLASS}>{t('applications.detail.jdTitle')}</span>
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
          <span className={LABEL_CLASS}>{t('applications.detail.answersTitle')}</span>
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
