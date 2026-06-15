import { HelpCircle } from 'lucide-react';

import type { InterviewQuestion } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Accordion } from '@ajh/ui';

import { INTERVIEW_AUDIENCES } from '@/lib/generate';

interface Props {
  questions: InterviewQuestion[];
}

/**
 * Renders interview questions grouped by audience, one collapsible {@link Accordion}
 * section per interviewer that has questions (title = audience label · count). The
 * first section is open by default. Shared by the Interview-prep tab and the
 * apply-time modal; the caller owns the empty state (renders nothing when empty).
 */
export function InterviewQuestionsAccordion({ questions }: Props) {
  const { t } = useTranslation();

  const grouped = INTERVIEW_AUDIENCES.map((aud) => ({
    aud,
    items: questions.filter((q) => q.audience === aud),
  })).filter((g) => g.items.length > 0);

  if (grouped.length === 0) return null;

  return (
    <div className="space-y-2">
      {grouped.map(({ aud, items }, i) => (
        <Accordion
          key={aud}
          defaultOpen={i === 0}
          title={`${t(`applications.detail.interview.audience.${aud}`)} · ${items.length}`}
          content={
            <ul className="space-y-3">
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
          }
        />
      ))}
    </div>
  );
}
