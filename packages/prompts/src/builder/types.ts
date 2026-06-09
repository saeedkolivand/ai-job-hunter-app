/**
 * Structured interview answers — the grounding source for the Resume Builder
 * (#1 / phase B9). The builder is a deterministic, offline questionnaire: the
 * user fills these fields, then a SINGLE AI pass synthesizes a complete résumé
 * from them (see `builder-prompt.ts`). No per-question LLM call.
 *
 * Contact identity (email / phone / location / profile links) is NOT here — it
 * lives in the authoritative contact profile and is applied to the export header
 * automatically (see the contact-profile docs). These answers carry only the
 * résumé BODY content plus the candidate's name/headline for prompt context.
 */

/** One work-experience entry (repeatable). */
export interface InterviewExperience {
  title: string;
  company: string;
  /** City / country, optional. */
  location?: string;
  /** Free-text start, e.g. "Jan 2021" — the synthesis normalizes the format. */
  startDate: string;
  /** Free-text end; empty (or `current`) renders as the localized "Present". */
  endDate: string;
  /** Current role — renders the target language's word for "Present". */
  current?: boolean;
  /** Achievement bullets (the candidate's own facts). */
  bullets: string[];
}

/** One education entry (repeatable). */
export interface InterviewEducation {
  /** Degree / qualification, e.g. "MSc Computer Science". */
  degree: string;
  institution: string;
  location?: string;
  startDate?: string;
  endDate?: string;
  /** Optional extras — honors, thesis, GPA, relevant coursework. */
  details?: string;
}

/** One project (optional, repeatable). The link survives inline (#18). */
export interface InterviewProject {
  name: string;
  description?: string;
  /** Repo / live / case-study URL — kept inline as `[name](link)`. */
  link?: string;
}

/** One publication (optional, repeatable) — academic résumés. DOI/url is a body link (#18). */
export interface InterviewPublication {
  title: string;
  /** Journal / conference / venue. */
  venue?: string;
  year?: string;
  /** DOI or article URL — kept inline. */
  link?: string;
}

/** A generic dated line — used for awards and volunteering (optional). */
export interface InterviewEntry {
  title: string;
  detail?: string;
  year?: string;
}

/**
 * The complete set of interview answers. Only `experience`, `education`, and
 * `skills` are core; everything else is optional and rendered only when present.
 * This shape is the single source shared by the prompt layer and the renderer's
 * `resumeBuilder` session slice.
 */
export interface InterviewAnswers {
  /** Candidate full name (also passed as `meta.candidateName`). */
  fullName: string;
  /** Optional target role / professional headline, informs the summary. */
  headline?: string;
  /**
   * Candidate-written professional summary. Kept (lightly polished) when set;
   * derived from the other answers when empty. Never replaced with a generic one.
   */
  summary?: string;
  experience: InterviewExperience[];
  education: InterviewEducation[];
  /** Skills as discrete entries — grouped ATS-style by the synthesis. */
  skills: string[];
  projects?: InterviewProject[];
  publications?: InterviewPublication[];
  awards?: InterviewEntry[];
  volunteer?: InterviewEntry[];
  /** Spoken languages, e.g. "English (native)", "German (B2)". */
  languages?: string[];
  certifications?: string[];
}
