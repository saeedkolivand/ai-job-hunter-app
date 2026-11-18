/**
 * "What an ATS sees": a strict, rigid-parser reading (Workday/Taleo style) of the
 * text re-extracted from an exported résumé PDF/DOCX.
 *
 * Advisory only: it never blocks or fails an export. Every shape is
 * machine-readable (enums, no prose), so an agent can reason over it directly.
 */

/** Whether the parser could read a field. */
export type AtsStatus = 'found' | 'missing' | 'suspicious';

/**
 * Why a field is `missing` or `suspicious`. Absent when `found`.
 * - `notFound`: no such content in the extracted text.
 * - `unparseable`: date-shaped text that is not a real month/year (`Jan '21`, `03/21`).
 * - `datesInverted`: the end date precedes the start date.
 * - `wrapped`: a phone number was split across two extracted lines.
 * - `nonstandardHeading`: an all-caps heading outside the standard section vocabulary.
 * - `interleaved`: body content was extracted before the header block (two-column interleave).
 */
export type AtsReason =
  'notFound' | 'unparseable' | 'datesInverted' | 'wrapped' | 'nonstandardHeading' | 'interleaved';

/** One extracted field. */
export interface AtsField {
  status: AtsStatus;
  /**
   * What the parser read. Dates are `YYYY-MM`, `YYYY` or (end only) `present`; an
   * unparseable date keeps the text as written. For `readingOrder` it is the first
   * extracted line when interleaved. Absent when nothing was read.
   */
  value?: string;
  /** Set exactly when `status` is not `found`. */
  reason?: AtsReason;
}

/** The contact fields read from the header block. */
export interface AtsContact {
  name: AtsField;
  email: AtsField;
  phone: AtsField;
  location: AtsField;
}

/** Language-independent section bucket of a heading. */
export type AtsSectionKind =
  'summary' | 'experience' | 'education' | 'projects' | 'skills' | 'other';

/**
 * A section heading as the parser sees it. A core kind (`experience`, `education`,
 * `skills`) that no heading maps to appears once with `status: 'missing'` and no `heading`.
 */
export interface AtsSection {
  kind: AtsSectionKind;
  /** The heading text as extracted; absent for a missing core section. */
  heading?: string;
  status: AtsStatus;
  reason?: AtsReason;
}

/** One work-experience entry. */
export interface AtsRole {
  title: AtsField;
  company: AtsField;
  /** Value `YYYY-MM` or `YYYY`. */
  start: AtsField;
  /** Value `YYYY-MM`, `YYYY` or `present`. */
  end: AtsField;
}

/** Field counts over the whole view (contact, reading order, sections, role fields). */
export interface AtsSummary {
  found: number;
  suspicious: number;
  missing: number;
  /** `found + suspicious + missing`. */
  total: number;
}

/** The strict parse view of one exported résumé. */
export interface AtsView {
  contact: AtsContact;
  sections: AtsSection[];
  roles: AtsRole[];
  /**
   * `found` = single-column order. `suspicious` with reason `interleaved` = the first
   * line is a section heading, or the candidate name / email line comes after the
   * first standard heading (what a two-column interleave extracts as). Without the
   * source name (the file command) the name condition is not judged. `missing` with
   * reason `notFound` = no standard heading was found, so no order could be confirmed.
   */
  readingOrder: AtsField;
  summary: AtsSummary;
}

export interface ExportContract {
  /**
   * Read a previously exported résumé PDF or DOCX and return its strict ATS view.
   * `path` is an absolute path to an existing `.pdf` or `.docx` file no larger than a
   * résumé import accepts (no `..` segments); anything else is a validation error. Read-only and advisory.
   */
  atsView(path: string): Promise<AtsView>;
}

export const EXPORT_CHANNELS = {
  atsView: 'export:ats_view',
} as const;
