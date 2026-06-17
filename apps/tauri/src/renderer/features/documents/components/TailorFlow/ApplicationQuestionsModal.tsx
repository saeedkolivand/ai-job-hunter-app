import { Check, Copy, HelpCircle, Plus, Sparkles, X } from 'lucide-react';
import { useState } from 'react';

import { APPLICATION_QUESTIONS } from '@ajh/prompts/generate';
import { useTranslation } from '@ajh/translations';
import { Button, Input, ModalShell } from '@ajh/ui';

import { MAX_CUSTOM_QUESTION_LEN } from './useApplicationAnswers';

interface Props {
  selected: Set<string>;
  toggle: (id: string) => void;
  custom: { id: string; question: string }[];
  addCustom: (text: string) => void;
  removeCustom: (id: string) => void;
  answers: Record<string, string>;
  generating: boolean;
  error: string | null;
  generate: () => void;
  canGenerate: boolean;
  onClose: () => void;
}

/**
 * "Application questions" assistant lifted out of the cramped results stack into a
 * button-triggered modal (mirrors {@link ReferralModal}). The stateful hook lives
 * in {@link ApplyPage} and is passed in as props, so closing the modal unmounts
 * this body WITHOUT losing the user's picks/answers or interrupting generation.
 * The user picks from a curated list and the app drafts résumé-grounded answers.
 */
export function ApplicationQuestionsModal({
  selected,
  toggle,
  custom,
  addCustom,
  removeCustom,
  answers,
  generating,
  error,
  generate,
  canGenerate,
  onClose,
}: Props) {
  const { t } = useTranslation();
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [draft, setDraft] = useState('');

  const copy = async (id: string, text: string) => {
    await navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1500);
  };

  const submitCustom = () => {
    if (!draft.trim()) return;
    addCustom(draft);
    setDraft('');
  };

  // Shared answer + Copy block — reused by predefined and custom rows.
  const answerBlock = (id: string, answer: string) => (
    <div className="px-2 pb-2 pl-7">
      <div className="relative rounded-md border border-white/[0.05] bg-white/[0.03] px-2.5 py-2">
        <p className="whitespace-pre-wrap pr-6 text-[11px] leading-relaxed text-foreground/70">
          {answer}
        </p>
        <Button
          variant="unstyled"
          type="button"
          onClick={() => void copy(id, answer)}
          title={t('autopilot.apply.questions.copy')}
          aria-label={t('autopilot.apply.questions.copy')}
          className="absolute right-1.5 top-1.5 rounded p-0.5 text-foreground/30 transition-colors hover:text-foreground/70"
        >
          {copiedId === id ? <Check size={11} /> : <Copy size={11} />}
        </Button>
      </div>
    </div>
  );

  return (
    <ModalShell
      open
      onClose={onClose}
      maxWidth="max-w-lg"
      // Second-layer modal opened from the ApplyPage — sit above the default
      // modal layer (600) so it never renders under its parent.
      zIndex={650}
      ariaLabelledby="application-questions-modal-title"
      header={
        <div className="flex items-start justify-between gap-3 border-b border-white/[0.08] px-5 py-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <HelpCircle size={14} className="shrink-0 text-brand-soft" />
              <span
                id="application-questions-modal-title"
                className="truncate text-sm font-semibold text-foreground/85"
              >
                {t('autopilot.apply.questions.title')}
              </span>
              {selected.size > 0 && (
                <span className="shrink-0 rounded-full bg-brand/15 px-1.5 py-0.5 text-[9px] text-brand-soft">
                  {selected.size}
                </span>
              )}
            </div>
            <div className="mt-0.5 truncate text-[11px] text-foreground/40">
              {t('autopilot.apply.questions.hint')}
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
      <div className="space-y-2 px-5 py-4">
        <div className="space-y-1.5">
          {APPLICATION_QUESTIONS.map((q) => {
            const answer = answers[q.id];
            return (
              <div key={q.id} className="rounded-md bg-white/[0.02]">
                <label className="flex cursor-pointer items-start gap-2 px-2 py-1.5">
                  <input
                    type="checkbox"
                    checked={selected.has(q.id)}
                    onChange={() => toggle(q.id)}
                    className="mt-0.5 accent-brand"
                  />
                  <span className="text-[11px] text-foreground/75">{q.question}</span>
                </label>
                {answer && answerBlock(q.id, answer)}
              </div>
            );
          })}
        </div>

        {/* Your own questions */}
        <div className="space-y-1.5 pt-1">
          <div className="px-2 text-[10px] font-medium uppercase tracking-wide text-foreground/40">
            {t('autopilot.apply.questions.customLabel')}
          </div>

          {custom.map((q) => {
            const answer = answers[q.id];
            return (
              <div key={q.id} className="rounded-md bg-white/[0.02]">
                <div className="flex items-start gap-2 px-2 py-1.5">
                  <span className="flex-1 text-[11px] text-foreground/75">{q.question}</span>
                  <Button
                    variant="unstyled"
                    type="button"
                    onClick={() => removeCustom(q.id)}
                    aria-label={t('autopilot.apply.questions.remove')}
                    className="shrink-0 rounded p-0.5 text-foreground/30 transition-colors hover:text-foreground/70"
                  >
                    <X size={12} />
                  </Button>
                </div>
                {answer && answerBlock(q.id, answer)}
              </div>
            );
          })}

          <div className="flex items-center gap-1.5 px-2">
            <Input
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  submitCustom();
                }
              }}
              placeholder={t('autopilot.apply.questions.customPlaceholder')}
              maxLength={MAX_CUSTOM_QUESTION_LEN}
              className="h-8 flex-1 text-[11px]"
            />
            <Button
              type="button"
              onClick={submitCustom}
              disabled={!draft.trim()}
              className="h-8 shrink-0 px-2"
            >
              <Plus size={12} />
              {t('autopilot.apply.questions.add')}
            </Button>
          </div>
        </div>

        {error && <p className="text-[11px] text-red-300/80">{error}</p>}

        <Button
          variant="primary"
          loading={generating}
          disabled={!canGenerate || generating}
          onClick={() => void generate()}
          className="w-full justify-center"
        >
          {!generating && <Sparkles size={12} />}
          {generating
            ? t('autopilot.apply.questions.generating')
            : t('autopilot.apply.questions.generate')}
        </Button>
      </div>
    </ModalShell>
  );
}
