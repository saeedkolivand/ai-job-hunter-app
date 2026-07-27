import type { ApplicationAnswer, InterviewQuestion } from '../../types/index.js';

export interface AiGenerationRecord {
  id: string;
  createdAt: number;
  candidateName: string;
  jobTitle: string;
  companyName: string;
  resumeLanguage: string;
  jobAdLanguage: string;
  targetLanguage: string;
  mismatch: boolean;
  topRequirements: string[];
  mode: string;
  resumeText: string;
  coverLetterText: string;
  jobAd: string;
  /** The job this generation targets — links the record to an autopilot found job. */
  jobUrl: string;
  /** The board the job came from (e.g. "linkedin"). */
  board: string;
  /** Answered application questions (the questions assistant), if any. */
  applicationAnswers: ApplicationAnswer[];
  /** The company-research brief used for this application, if any. */
  companyBrief: string;
  /** AI-suggested questions the candidate can ASK the interviewer, if any. */
  interviewQuestions: InterviewQuestion[];
  /**
   * The persisted apply-by-email draft (subject line + body). Optional because
   * records serialised before these columns existed — e.g. an older exported
   * backup replayed through a test fixture — carry neither field.
   */
  emailSubject?: string;
  emailBody?: string;
  /**
   * Parent Application FK — set at save time (and backfilled at boot for legacy
   * rows). The Application detail page joins this generation's docs by this id, not
   * by url, because the Application stores the NORMALIZED url and the generation the
   * RAW one (they never match for query-id boards like Indeed). Absent when unlinked.
   */
  applicationId?: string;
}

export interface AiGenerationSaveRequest {
  candidateName: string;
  jobTitle: string;
  companyName: string;
  resumeLanguage: string;
  jobAdLanguage: string;
  targetLanguage: string;
  mismatch: boolean;
  topRequirements: string[];
  mode: string;
  resumeText: string;
  coverLetterText: string;
  jobAd: string;
  /** The job this generation targets (marks the autopilot found job "applied"). */
  jobUrl?: string;
  /** The board the job came from. */
  board?: string;
  /** Answered application questions to persist on the (per-job) record. */
  applicationAnswers?: ApplicationAnswer[];
  /** The company-research brief used, persisted for audit. */
  companyBrief?: string;
  /** AI-suggested interview questions to persist on the (per-job) record. */
  interviewQuestions?: InterviewQuestion[];
  /**
   * The apply-by-email draft to persist on the (per-job) record. Merged like
   * `coverLetterText`: a non-blank value overwrites the stored draft, a blank
   * one leaves it untouched — so a résumé/answers save can't wipe the email.
   */
  emailSubject?: string;
  emailBody?: string;
}

/**
 * Edit the résumé/cover-letter text of an existing saved generation, selected by
 * `id`. Unlike {@link AiGenerationSaveRequest} (a per-job merge-upsert that keeps
 * existing non-empty text), this is a direct overwrite — so a user editing a
 * saved generation can blank out or fully replace the text. Each text field is
 * optional; an absent field is left unchanged.
 */
export interface AiGenerationUpdateRequest {
  id: string;
  resumeText?: string;
  coverLetterText?: string;
}

/**
 * Result of `save` — matches the Rust `{ id, success } | { error }` shape the
 * command actually returns. Every field is optional because the two arms are
 * disjoint: a failed save carries only `error`, so a caller that wants to know
 * whether the write landed must check it (the command reports failure in-band
 * rather than rejecting the promise).
 */
export interface AiGenerationSaveResult {
  id?: string;
  success?: boolean;
  error?: string;
}

export interface AiGenerationsContract {
  list(): Promise<AiGenerationRecord[]>;
  save(req: AiGenerationSaveRequest): Promise<AiGenerationSaveResult>;
  update(req: AiGenerationUpdateRequest): Promise<void>;
  remove(id: string): Promise<void>;
  removeBulk(ids: string[]): Promise<void>;
}

export const AI_GENERATIONS_CHANNELS = {
  list: 'aiGenerations:list',
  save: 'aiGenerations:save',
  update: 'aiGenerations:update',
  remove: 'aiGenerations:remove',
  removeBulk: 'aiGenerations:removeBulk',
} as const;
