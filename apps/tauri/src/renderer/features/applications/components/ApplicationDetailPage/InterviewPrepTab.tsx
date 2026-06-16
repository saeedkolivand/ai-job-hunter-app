import { MessagesSquare, Sparkles } from 'lucide-react';

import type { AiGenerationRecord, Application, InterviewQuestion } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, CardSkeleton, EmptyState, TextArea } from '@ajh/ui';

import { AudienceSelector } from '@/components/interview/AudienceSelector';
import { InterviewQuestionsAccordion } from '@/components/interview/InterviewQuestionsAccordion';
import { useCanUseAI, useSelectedModel } from '@/components/ui/ModelSelector';
import { useDefaultResumeId } from '@/features/jobs/hooks/useDefaultResumeId';
import { useInterviewQuestions } from '@/hooks/use-interview-questions';
import { useDocuments, useDocumentText, useResolveJobUrl } from '@/services';

interface Props {
  application: Application;
  matchingGenerations: AiGenerationRecord[];
}

/**
 * Interview-prep tab — AI-suggested questions the candidate can ASK the
 * interviewer, grouped by audience. Self-sources the résumé (default doc) + job
 * description (saved generation or resolved from the URL) and persists generated
 * questions onto the per-job aggregate via {@link useInterviewQuestions}.
 */
export function InterviewPrepTab({ application, matchingGenerations }: Props) {
  const { t } = useTranslation();
  const model = useSelectedModel();
  const { canUse } = useCanUseAI();

  const docsQuery = useDocuments();
  const defaultResumeId = useDefaultResumeId();
  const resumeQuery = useDocumentText(defaultResumeId);

  const saved = matchingGenerations[0];
  const initialDesc = (saved?.jobAd ?? '').trim();
  const resolved = useResolveJobUrl(application.jobUrl, !initialDesc);
  const jobDesc = initialDesc || (resolved.data?.description ?? '').trim();
  const hasDesc = jobDesc.length > 0;
  const resume = (resumeQuery.data ?? '') || (saved?.resumeText ?? '');

  const iq = useInterviewQuestions({
    resume,
    jobDesc,
    model,
    canUse,
    hasDesc,
    jobUrl: application.jobUrl,
    board: application.board ?? '',
  });

  if (docsQuery.isLoading || (!!defaultResumeId && resumeQuery.isLoading)) {
    return (
      <div className="h-full overflow-y-auto px-6 py-5">
        <CardSkeleton />
      </div>
    );
  }

  // Freshly-generated set wins; before any generation, show the saved set (if any).
  const displayed: InterviewQuestion[] =
    iq.questions.length > 0 ? iq.questions : (saved?.interviewQuestions ?? []);

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Toolbar — audience selector + seed topics + generate */}
      <div className="shrink-0 space-y-2 border-b border-[var(--border-soft)] px-8 py-3">
        <span className="block text-xs font-medium text-foreground/70">
          {t('applications.detail.interview.audienceLabel')}
        </span>
        <AudienceSelector selected={iq.audiences} onToggle={iq.toggleAudience} />
        <label htmlFor="iq-seeds" className="block pt-1 text-xs font-medium text-foreground/70">
          {t('applications.detail.interview.seedLabel')}
        </label>
        <div className="flex items-start gap-2">
          <TextArea
            id="iq-seeds"
            variant="glass"
            rows={2}
            value={iq.seedTopics}
            onChange={(e) => iq.setSeedTopics(e.target.value)}
            placeholder={t('applications.detail.interview.seedPlaceholder')}
            className="flex-1"
          />
          <Button
            variant="primary"
            disabled={!iq.canGenerate || iq.generating}
            onClick={() => void iq.generate()}
            className="shrink-0 gap-1.5"
          >
            <Sparkles size={13} />
            {iq.generating
              ? t('applications.detail.interview.generating')
              : displayed.length > 0
                ? t('applications.detail.interview.regenerate')
                : t('applications.detail.interview.generate')}
          </Button>
        </div>
        {!canUse && (
          <p className="text-[11px] text-amber-400/80">
            {t('applications.detail.interview.needsModel')}
          </p>
        )}
        {canUse && !hasDesc && (
          <p className="text-[11px] text-amber-400/80">
            {t('applications.detail.interview.needsJob')}
          </p>
        )}
        {iq.needsResearchKey && (
          <p className="text-[11px] text-amber-400/70">{t('aiGenerate.research.ollamaKeyHint')}</p>
        )}
        {iq.error && <p className="text-[11px] text-red-400/80">{iq.error}</p>}
      </div>

      {/* Body */}
      <div className="min-h-0 flex-1 overflow-y-auto px-8 py-5">
        {displayed.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <EmptyState
              icon={MessagesSquare}
              title={t('applications.detail.interview.empty')}
              description={t('applications.detail.interview.emptyDesc')}
              className="py-12"
            />
          </div>
        ) : (
          <InterviewQuestionsAccordion questions={displayed} />
        )}
      </div>
    </div>
  );
}
