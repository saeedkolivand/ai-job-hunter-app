import { HelpCircle, MessagesSquare, Sparkles, X } from 'lucide-react';

import type { InterviewQuestion } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Button, EmptyState, ModalShell, TextArea } from '@ajh/ui';

/** Group order for the audience sections (matches the prompt's audience tags). */
const AUDIENCE_ORDER = ['recruiter', 'hiringManager', 'team', 'leadership', 'general'] as const;

interface Props {
  seedTopics: string;
  setSeedTopics: (v: string) => void;
  questions: InterviewQuestion[];
  generating: boolean;
  error: string | null;
  generate: () => void;
  canGenerate: boolean;
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
  questions,
  generating,
  error,
  generate,
  canGenerate,
  onClose,
}: Props) {
  const { t } = useTranslation();
  const grouped = AUDIENCE_ORDER.map((aud) => ({
    aud,
    items: questions.filter((q) => q.audience === aud),
  })).filter((g) => g.items.length > 0);

  return (
    <ModalShell
      open
      onClose={onClose}
      maxWidth="max-w-lg"
      // Second-layer modal opened from the apply toolbar — sit above the default
      // modal layer (600) so it never renders under its parent.
      zIndex={650}
      ariaLabelledby="interview-questions-modal-title"
    >
      <div className="flex max-h-[85vh] flex-col">
        {/* Header */}
        <div className="flex items-start justify-between gap-3 border-b border-white/[0.08] px-5 py-4">
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

        {/* Body */}
        <div className="flex-1 space-y-3 overflow-y-auto px-5 py-4">
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

          {error && <p className="text-[11px] text-red-300/80">{error}</p>}

          {questions.length === 0
            ? !generating && (
                <EmptyState
                  icon={MessagesSquare}
                  title={t('applications.detail.interview.empty')}
                  className="py-6"
                />
              )
            : grouped.map(({ aud, items }) => (
                <div key={aud} className="space-y-2">
                  <span className="block text-[10px] font-semibold uppercase tracking-[0.16em] text-foreground/45">
                    {t(`applications.detail.interview.audience.${aud}`)}
                  </span>
                  <ul className="space-y-2">
                    {items.map((q) => (
                      <li key={q.id} className="border-l border-white/[0.06] pl-3">
                        <p className="select-text text-[12px] leading-relaxed text-foreground/85">
                          {q.question}
                        </p>
                        {q.why && (
                          <p className="mt-0.5 flex items-start gap-1 text-[11px] leading-relaxed text-foreground/45">
                            <HelpCircle size={11} className="mt-0.5 shrink-0 text-brand-soft/70" />
                            {q.why}
                          </p>
                        )}
                      </li>
                    ))}
                  </ul>
                </div>
              ))}

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
      </div>
    </ModalShell>
  );
}
