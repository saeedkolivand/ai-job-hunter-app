import { AlertCircle, CheckCircle2, MessagesSquare, Sparkles } from 'lucide-react';
import { useEffect, useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button, cn, EmptyState, RowSkeleton, StreamingText, TextArea } from '@ajh/ui';

import type { LikelyQuestion, StarCompleteness, StarFeedback } from '@/lib/generate';

/** One question's live STAR-feedback request state (mirrors `useInterviewPractice`'s
 *  per-question map — kept local so this component has no hook dependency). */
export interface PracticeFeedbackEntry {
  text: string;
  feedback: StarFeedback | null;
  loading: boolean;
  error: string | null;
}

interface Props {
  questions: LikelyQuestion[];
  generating: boolean;
  error: string | null;
  canGenerate: boolean;
  canUse: boolean;
  hasDesc: boolean;
  onGenerate: () => void;
  feedback: Record<string, PracticeFeedbackEntry>;
  onGetFeedback: (question: LikelyQuestion, answer: string) => void;
}

const STAR_FIELDS: (keyof StarCompleteness)[] = ['situation', 'task', 'action', 'result'];

/**
 * Practice-answers mode of the interview-prep tab — lists AI-generated likely
 * questions for this role, each with a freeform answer box and a "Get feedback"
 * action that streams STAR-rubric coaching. Session-only: the caller
 * ({@link useInterviewPractice}) never persists questions, answers, or feedback.
 */
export function InterviewPracticePanel({
  questions,
  generating,
  error,
  canGenerate,
  canUse,
  hasDesc,
  onGenerate,
  feedback,
  onGetFeedback,
}: Props) {
  const { t } = useTranslation();
  const [answers, setAnswers] = useState<Record<string, string>>({});

  // A Regenerate swaps in a brand-new `questions` array (see useInterviewPractice).
  // Typed answers are keyed by question id and MUST NOT carry over onto the new
  // set — clear them whenever the array identity changes.
  useEffect(() => {
    setAnswers({});
  }, [questions]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 space-y-2 border-b border-[var(--border-soft)] px-8 py-3">
        <p className="text-xs text-foreground/60">
          {t('applications.detail.interview.practice.hint')}
        </p>
        <Button
          variant="primary"
          disabled={!canGenerate || generating}
          onClick={onGenerate}
          className="gap-1.5"
        >
          <Sparkles size={13} />
          {generating
            ? t('applications.detail.interview.practice.generating')
            : questions.length > 0
              ? t('applications.detail.interview.practice.regenerate')
              : t('applications.detail.interview.practice.generate')}
        </Button>
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
        {error && <p className="text-[11px] text-red-400/80">{error}</p>}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-8 py-5">
        {questions.length === 0 ? (
          generating ? (
            <div className="space-y-2">
              <RowSkeleton />
              <RowSkeleton />
              <RowSkeleton />
            </div>
          ) : (
            <div className="flex h-full items-center justify-center">
              <EmptyState
                icon={MessagesSquare}
                title={t('applications.detail.interview.practice.empty')}
                description={t('applications.detail.interview.practice.emptyDesc')}
                className="py-12"
              />
            </div>
          )
        ) : (
          <ul className="space-y-3">
            {questions.map((q) => (
              <PracticeQuestionItem
                key={q.id}
                question={q}
                answer={answers[q.id] ?? ''}
                onAnswerChange={(text) => setAnswers((prev) => ({ ...prev, [q.id]: text }))}
                feedback={feedback[q.id]}
                canUse={canUse}
                onGetFeedback={() => onGetFeedback(q, answers[q.id] ?? '')}
              />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

interface ItemProps {
  question: LikelyQuestion;
  answer: string;
  onAnswerChange: (text: string) => void;
  feedback: PracticeFeedbackEntry | undefined;
  canUse: boolean;
  onGetFeedback: () => void;
}

function PracticeQuestionItem({
  question,
  answer,
  onAnswerChange,
  feedback,
  canUse,
  onGetFeedback,
}: ItemProps) {
  const { t } = useTranslation();
  const canAsk = canUse && answer.trim().length > 0 && !feedback?.loading;

  return (
    <li className="surface-card space-y-2 rounded-lg p-3">
      <div className="flex items-start justify-between gap-2">
        <p className="select-text text-[12px] leading-relaxed text-foreground/85">
          {question.question}
        </p>
        <span className="shrink-0 rounded-full bg-foreground/[0.06] px-2 py-0.5 text-[10px] text-foreground/50">
          {t(`applications.detail.interview.practice.type.${question.type}`)}
        </span>
      </div>
      <TextArea
        variant="glass"
        rows={3}
        value={answer}
        onChange={(e) => onAnswerChange(e.target.value)}
        placeholder={t('applications.detail.interview.practice.answerPlaceholder')}
        aria-label={question.question}
      />
      <Button
        variant="default"
        size="sm"
        disabled={!canAsk}
        onClick={onGetFeedback}
        className="gap-1.5"
      >
        <Sparkles size={12} />
        {feedback?.loading
          ? t('applications.detail.interview.practice.gettingFeedback')
          : t('applications.detail.interview.practice.getFeedback')}
      </Button>
      {feedback?.error && <p className="text-[11px] text-red-400/80">{feedback.error}</p>}
      {feedback?.loading && !feedback.feedback && (
        <StreamingText
          text={feedback.text}
          isStreaming
          className="border-t border-[var(--border-soft)] pt-2"
        />
      )}
      {feedback?.feedback && <StarFeedbackRubric feedback={feedback.feedback} />}
    </li>
  );
}

function StarFeedbackRubric({ feedback }: { feedback: StarFeedback }) {
  const { t } = useTranslation();

  return (
    <div className="space-y-3 border-t border-[var(--border-soft)] pt-3">
      {feedback.strengths.length > 0 && (
        <div>
          <p className="text-[11px] font-medium text-foreground/70">
            {t('applications.detail.interview.practice.feedback.strengths')}
          </p>
          <ul className="mt-1 space-y-0.5">
            {feedback.strengths.map((s, i) => (
              <li
                // Index key: AI-generated strings can legitimately repeat, so
                // content can't be a stable/unique key here.
                key={i}
                className="flex items-start gap-1 text-[11px] leading-relaxed text-foreground/60"
              >
                <CheckCircle2 size={11} className="mt-0.5 shrink-0 text-emerald-400/70" />
                {s}
              </li>
            ))}
          </ul>
        </div>
      )}
      {feedback.gaps.length > 0 && (
        <div>
          <p className="text-[11px] font-medium text-foreground/70">
            {t('applications.detail.interview.practice.feedback.gaps')}
          </p>
          <ul className="mt-1 space-y-0.5">
            {feedback.gaps.map((g, i) => (
              <li
                // Index key: AI-generated strings can legitimately repeat, so
                // content can't be a stable/unique key here.
                key={i}
                className="flex items-start gap-1 text-[11px] leading-relaxed text-foreground/60"
              >
                <AlertCircle size={11} className="mt-0.5 shrink-0 text-amber-400/70" />
                {g}
              </li>
            ))}
          </ul>
        </div>
      )}
      <div>
        <p className="text-[11px] font-medium text-foreground/70">
          {t('applications.detail.interview.practice.feedback.starCompleteness')}
        </p>
        <div className="mt-1 flex flex-wrap gap-1.5">
          {STAR_FIELDS.map((key) => (
            <span
              key={key}
              className={cn(
                'rounded-full px-2 py-0.5 text-[10px]',
                feedback.star[key]
                  ? 'bg-emerald-500/10 text-emerald-400/80'
                  : 'bg-foreground/[0.06] text-foreground/45'
              )}
            >
              {t(`applications.detail.interview.practice.feedback.${key}`)}:{' '}
              {feedback.star[key]
                ? t('applications.detail.interview.practice.feedback.present')
                : t('applications.detail.interview.practice.feedback.missing')}
            </span>
          ))}
        </div>
      </div>
      {feedback.rewrite && (
        <div>
          <p className="text-[11px] font-medium text-foreground/70">
            {t('applications.detail.interview.practice.feedback.rewrite')}
          </p>
          <p className="mt-1 select-text text-[12px] leading-relaxed text-foreground/80">
            {feedback.rewrite}
          </p>
        </div>
      )}
    </div>
  );
}
