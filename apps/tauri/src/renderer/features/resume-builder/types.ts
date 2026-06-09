import type { InterviewAnswers } from '@/lib/generate';

/** Common props every Resume Builder wizard step receives. */
export interface BuilderStepProps {
  answers: InterviewAnswers;
  /** Shallow-merge a patch into the interview answers (immutable update). */
  update: (patch: Partial<InterviewAnswers>) => void;
}
