import { Check, ChevronDown, Copy, HelpCircle, Sparkles } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';

import { APPLICATION_QUESTIONS } from '@ajh/prompts/generate';
import { Button, cn, transition } from '@ajh/ui';

import type { GenerationMeta } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import { useApplicationAnswers } from './useApplicationAnswers';

interface Props {
  resume: string;
  jobDesc: string;
  model: string;
  researchCompany: boolean;
  meta?: GenerationMeta | null;
  canUse: boolean;
  hasDesc: boolean;
  jobUrl: string;
  board: string;
}

/**
 * Optional "application questions" assistant inside the Apply modal: the user
 * picks from a curated list and the app drafts résumé-grounded answers (company
 * research shares the cover letter's opt-in toggle). Collapsed by default so it
 * never competes with the primary resume/cover flow.
 */
export function ApplicationQuestions({
  resume,
  jobDesc,
  model,
  researchCompany,
  meta,
  canUse,
  hasDesc,
  jobUrl,
  board,
}: Props) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const { selected, toggle, answers, generating, error, generate, canGenerate } =
    useApplicationAnswers({
      resume,
      jobDesc,
      model,
      researchCompany,
      meta,
      canUse,
      hasDesc,
      jobUrl,
      board,
    });

  const copy = async (id: string, text: string) => {
    await navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1500);
  };

  return (
    <div className="overflow-hidden rounded-lg border border-white/[0.06] bg-white/[0.02]">
      <Button
        variant="unstyled"
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center justify-between px-3 py-2.5 text-left"
      >
        <span className="flex items-center gap-2 text-[11px] font-medium text-foreground/75">
          <HelpCircle size={12} className="text-brand-soft" />
          {t('autopilot.apply.questions.title')}
          {selected.size > 0 && (
            <span className="rounded-full bg-brand/15 px-1.5 py-0.5 text-[9px] text-brand-soft">
              {selected.size}
            </span>
          )}
        </span>
        <ChevronDown
          size={13}
          className={cn('text-foreground/30 transition-transform', open && 'rotate-180')}
        />
      </Button>

      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={transition.fast}
            className="overflow-hidden"
          >
            <div className="space-y-2 border-t border-white/[0.05] px-3 py-3">
              <p className="text-[10px] text-foreground/40">
                {t('autopilot.apply.questions.hint')}
              </p>

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
                      {answer && (
                        <div className="px-2 pb-2 pl-7">
                          <div className="relative rounded-md border border-white/[0.05] bg-white/[0.03] px-2.5 py-2">
                            <p className="whitespace-pre-wrap pr-6 text-[11px] leading-relaxed text-foreground/70">
                              {answer}
                            </p>
                            <Button
                              variant="unstyled"
                              type="button"
                              onClick={() => void copy(q.id, answer)}
                              title={t('autopilot.apply.questions.copy')}
                              className="absolute right-1.5 top-1.5 rounded p-0.5 text-foreground/30 transition-colors hover:text-foreground/70"
                            >
                              {copiedId === q.id ? <Check size={11} /> : <Copy size={11} />}
                            </Button>
                          </div>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>

              {error && <p className="text-[11px] text-red-300/80">{error}</p>}

              <Button
                variant="glass"
                size="sm"
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
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
