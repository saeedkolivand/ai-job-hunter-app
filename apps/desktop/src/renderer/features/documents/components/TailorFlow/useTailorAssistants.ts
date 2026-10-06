import type { AutopilotFoundJob } from '@ajh/shared';

import { useInterviewQuestions } from '@/hooks/use-interview-questions';

import { useApplicationAnswers } from './useApplicationAnswers';
import { useJobAdSummary } from './useJobAdSummary';
import type { TailorPipelineSession } from './useTailorPipeline';

interface Params {
  job: AutopilotFoundJob;
  resume: string;
  jobDesc: string;
  model: string;
  researchCompany: boolean;
  gen: Pick<TailorPipelineSession, 'meta' | 'targetLanguageConfident'>;
  canUse: boolean;
  hasDesc: boolean;
  jobUrl: string;
  board: string;
  applicationId?: string;
  initialSummary?: string;
}

/**
 * The three AI helpers that sit beside the run — job-ad summary, application
 * answers and interviewer questions. They live here (not in their panels) so
 * a modal can fully unmount on close without losing the user's picks/answers
 * and an in-flight generation keeps running while it is closed.
 */
export function useTailorAssistants(p: Params) {
  // `common` is what every assistant reads; each hook picks the fields it needs.
  const { job, gen, researchCompany, applicationId, initialSummary, ...common } = p;
  const { meta, targetLanguageConfident } = gen;

  // Lazy, résumé-independent AI summary of the job ad (shared by the wizard's
  // job-ad step and the results job-ad tab). Reuses the flow's detected meta.
  const jobAdSummary = useJobAdSummary({ ...common, meta, applicationId, initialSummary });

  const questions = useApplicationAnswers({
    ...common,
    researchCompany,
    meta,
    targetLanguageConfident,
    salaryMin: job.salaryMin,
    salaryMax: job.salaryMax,
    salaryCurrency: job.salaryCurrency,
  });

  // "Questions to ask the interviewer" — the second assistant. Same inputs; it
  // always gathers its own company/role research (not gated on the toggle).
  const interview = useInterviewQuestions({ ...common, meta, targetLanguageConfident });

  return { jobAdSummary, questions, interview };
}
