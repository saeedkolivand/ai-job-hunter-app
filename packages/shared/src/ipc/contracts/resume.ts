import type { ResumeValidateContentRequest } from '../../schemas/index.js';

export interface ResumeContract {
  /** Extract plain text from an uploaded resume/job-ad file (pdf, docx, txt, md). */
  extractText(req: { name: string; bytes: Uint8Array }): Promise<{ text: string }>;
  /**
   * Deterministic content-quality checks (factual accuracy, ATS structure,
   * AI-voice tells) on an already-generated résumé/letter against its source
   * résumé and the job ad. Pure and fast — no AI call, safe to call on every
   * save. See `validate::content::validate_content` (Rust, L1).
   *
   * `req.docKind` must be exactly `'resume'` or `'coverLetter'` — the Zod
   * `z.enum` here is renderer-side only; the Rust command rejects any other
   * value with a Validation error rather than guessing which ruleset to run.
   */
  validateContent(req: ResumeValidateContentRequest): Promise<ContentReportPayload>;
}

/**
 * Wire shape of Rust's `validate::content::ContentReport` — `ContentReport`
 * derives `Serialize` only (its `code` is a `&'static str`), so this is a hand
 * mirror rather than a generated type. `code` is the stable i18n key from
 * `CONTENT_ISSUE_CODES`; `section`/`evidence` serialize as `null`, not omitted
 * (no `skip_serializing_if` on those fields Rust-side).
 */
export interface ContentReportPayload {
  ok: boolean;
  issues: {
    severity: 'critical' | 'warning';
    code: string;
    section: string | null;
    message: string;
    evidence: string | null;
  }[];
  metrics: {
    keywordCoverage: number | null;
    /**
     * How many of the posting's top requirements the document evidences, or
     * `null` when nothing was measured — an uncomparable posting, an empty
     * requirements list, or a cover letter (which never runs the alignment
     * pass). Render the absent value as "—"; a `0` here would claim a
     * measurement that was never taken.
     */
    topRequirementHits: number | null;
    duplicateRatio: number;
    rolesSource: number;
    rolesOutput: number;
  };
}

export const RESUME_CHANNELS = {
  extractText: 'resume:extractText',
  validateContent: 'resume:validateContent',
} as const;
