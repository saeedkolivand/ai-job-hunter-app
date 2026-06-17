import { MessagesSquare, Sparkles, X } from 'lucide-react';

import type { InterviewQuestion } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, EmptyState, ModalShell, TextArea } from '@ajh/ui';

import { AudienceSelector } from '@/components/interview/AudienceSelector';
import { InterviewQuestionsAccordion } from '@/components/interview/InterviewQuestionsAccordion';

interface Props {
  seedTopics: string;
  setSeedTopics: (v: string) => void;
  /** Selected target interviewers. */
  audiences: string[];
  /** Toggle one audience on/off. */
  toggleAudience: (audience: string) => void;
  questions: InterviewQuestion[];
  generating: boolean;
  error: string | null;
  generate: () => void;
  canGenerate: boolean;
  /** Ollama-family provider without the web-search key — research won't be grounded. */
  needsResearchKey: boolean;
  onClose: () => void;
}

/**
 * "Questions to ask the interviewer" assistant in a button-triggered modal
 * (mirrors {@link ApplicationQuestionsModal}). The stateful {@link useInterviewQuestions}
 * hook lives in {@link TailorFlow} and is passed in as props, so closing the modal
 * unmounts this body WITHOUT losing the generated set or interrupting a run.
 */
export function InterviewQuestionsModal({
  seedTopics,
  setSeedTopics,
  audiences,
  toggleAudience,
  questions,
  generating,
  error,
  generate,
  canGenerate,
  needsResearchKey,
  onClose,
}: Props) {
  const { t } = useTranslation();

  return (
    <ModalShell
      open
      onClose={onClose}
      maxWidth="max-w-lg"
      // Second-layer modal opened from the apply toolbar — sit above the default
      // modal layer (600) so it never renders under its parent.
      zIndex={650}
      ariaLabelledby="interview-questions-modal-title"
      header={
        <div className="flex items-start justify-between gap-3 border-b border-[var(--border-soft)] px-5 py-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <MessagesSquare size={14} className="shrink-0 text-brand-soft" />
              <span
                id="interview-questions-modal-title"
                className="truncate text-sm font-semibold text-foreground/85"
              >
                {t('applications.detail.interview.title')}
              </span>
            </div>
            <div className="mt-0.5 truncate text-[11px] text-foreground/40">
              {t('applications.detail.interview.hint')}
            </div>
          </div>
          <Button
            onClick={onClose}
            aria-label={t('autopilot.referral.close')}
            className="h-auto shrink-0 border-transparent bg-transparent p-0 text-foreground/30 hover:text-foreground/60"
          >
            <X size={16} />
          </Button>
        </div>
      }
    >
      {/* Body */}
      <div className="space-y-3 px-5 py-4">
        <span className="block text-xs font-medium text-foreground/70">
          {t('applications.detail.interview.audienceLabel')}
        </span>
        <AudienceSelector selected={audiences} onToggle={toggleAudience} />

        <label htmlFor="iq-modal-seeds" className="block text-xs font-medium text-foreground/70">
          {t('applications.detail.interview.seedLabel')}
        </label>
        <TextArea
          id="iq-modal-seeds"
          variant="glass"
          rows={2}
          value={seedTopics}
          onChange={(e) => setSeedTopics(e.target.value)}
          placeholder={t('applications.detail.interview.seedPlaceholder')}
        />

        {needsResearchKey && (
          <p className="text-[11px] text-amber-300/70">{t('aiGenerate.research.ollamaKeyHint')}</p>
        )}
        {error && <p className="text-[11px] text-red-300/80">{error}</p>}

        {questions.length === 0 ? (
          !generating && (
            <EmptyState
              icon={MessagesSquare}
              title={t('applications.detail.interview.empty')}
              className="py-6"
            />
          )
        ) : (
          <InterviewQuestionsAccordion questions={questions} />
        )}

        <Button
          variant="primary"
          loading={generating}
          disabled={!canGenerate || generating}
          onClick={() => void generate()}
          className="w-full justify-center"
        >
          {!generating && <Sparkles size={12} />}
          {generating
            ? t('applications.detail.interview.generating')
            : questions.length > 0
              ? t('applications.detail.interview.regenerate')
              : t('applications.detail.interview.generate')}
        </Button>
      </div>
    </ModalShell>
  );
}
