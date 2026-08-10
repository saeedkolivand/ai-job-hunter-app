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
     *
     * Only meaningful next to `topRequirementsMeasured`: a bare "2" reads the
     * same for 2-of-2 and 2-of-10.
     */
    topRequirementHits: number | null;
    /**
     * The denominator for `topRequirementHits`: how many of the posting's top
     * requirements could be measured at all. `null` exactly when
     * `topRequirementHits` is — Rust produces the pair from one `Option` — so
     * one null check covers both. Lower than the requirements list whenever a
     * requirement has no extractable keywords ("Team player!"), and `0` when
     * none of them had any (the analysis produced requirements this kernel
     * cannot check — distinct from "no requirements", which is `null`).
     *
     * Optional in this mirror ONLY so that payload literals written before the
     * field existed keep type-checking; Rust always serializes it
     * (present-and-null, no `skip_serializing_if`). Read it as
     * `metrics.topRequirementsMeasured ?? null` and treat `undefined` as "not
     * measured".
     */
    topRequirementsMeasured?: number | null;
    duplicateRatio: number;
    rolesSource: number;
    rolesOutput: number;
  };
}

export const RESUME_CHANNELS = {
  extractText: 'resume:extractText',
  validateContent: 'resume:validateContent',
} as const;
