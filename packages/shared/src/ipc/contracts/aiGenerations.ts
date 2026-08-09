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
  /**
   * Serialized JSON wrapper `{schemaVersion, pipeline, generatedAt, resume?,
   * coverLetter?}` (this shape is renderer-owned) holding the deterministic
   * content-quality report(s) (`validate::content::ContentReport` per
   * sub-key). Each sub-report may carry its own `sourceTextHash` so the
   * renderer can flag it stale against the current résumé/letter text — the
   * Rust store never clears a report on a text edit, so staleness display is
   * entirely a renderer-side, read-time decision.
   *
   * Always present on a record returned from `list`/`save` (possibly `''` = no
   * report yet, or the row predates this field) — unlike on
   * {@link AiGenerationSaveRequest.qualityReport}, where it is genuinely
   * optional (omit to leave whatever report is already on the aggregate). A
   * save MERGES its incoming wrapper onto the existing one per TOP-LEVEL key:
   * a letter-only save overlays only `coverLetter` (plus the envelope fields)
   * and leaves a stored `resume` sub-report untouched, and vice versa. See
   * ADR-007 addendum — a manual text edit via {@link AiGenerationUpdateRequest}
   * deliberately never clears this.
   */
  qualityReport?: string;
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
  /**
   * Deterministic content-quality report wrapper to merge onto the aggregate
   * (see {@link AiGenerationRecord.qualityReport} for the shape and the
   * per-key merge rule). Renderer's job to compute it (typically right after a
   * resume/cover regeneration) and pass it here; an absent/empty value merges
   * nothing, leaving whatever report is already on the aggregate untouched.
   */
  qualityReport?: string;
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
 * Result of `save` — the Rust command reports failure IN-BAND (it resolves with
 * `{ error }` instead of rejecting), so a caller's `onError` never fires for a
 * store failure. Modelled as a union of the two disjoint arms the command
 * actually returns, which makes the compiler refuse a bare `result.id` until
 * the failure arm has been narrowed out (`'error' in result`) — the check is
 * otherwise trivially forgotten, which is exactly how it was missed here.
 */
export type AiGenerationSaveResult = { id: string; success: true } | { error: string };

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
