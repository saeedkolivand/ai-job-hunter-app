/**
 * Deterministic content-quality report for a just-generated résumé and/or
 * cover letter — the renderer-side glue around `resume:validateContent`
 * (`validate::content::validate_content`, Rust, L1: pure and fast, no AI
 * call). Called right before a save that writes `resumeText`/`coverLetterText`
 * so every such save carries a fresh report (Phase-1 plan's merge rule).
 *
 * Best-effort by design: validation must never block or fail a generation or
 * save. A per-doc failure degrades to "no report for that doc" (logged via
 * `console.warn`, never thrown) rather than surfacing a user-facing error.
 */
import type { ContentReportPayload } from '@ajh/shared/ipc';

import { getClient } from '../app-client';
import { errorDetail } from '../error-class';

/** Sentinel Rust's `pick_report` merge treats as "no report" alongside `''`. */
const EMPTY_REPORT_PLACEHOLDER = '{}';

/**
 * Wire shape persisted in `AiGenerationRecord.qualityReport` — an opaque JSON
 * string on the Rust side (the migration/merge logic never parses it), so this
 * wrapper is free to carry BOTH documents' reports plus provenance rather than
 * mirroring Rust's single-document `ContentReport` 1:1.
 */
export interface QualityReport {
  schemaVersion: 1;
  /** Which validation pipeline produced this — 'fast' is the only one today
   *  (the deterministic checks); reserved for a future slower/deeper pass. */
  pipeline: 'fast';
  generatedAt: number;
  resume?: ContentReportPayload;
  coverLetter?: ContentReportPayload;
  /**
   * djb2 hash of the EXACT text each sub-report validated. A reader compares
   * `hashText(currentText)` against this to detect the document has since
   * diverged (a hand-edit) WITHOUT keeping a full second copy of the text
   * around — this single mechanism covers both live staleness (compared
   * against the live session's `resumeOut`/`coverOut`) and persisted
   * staleness (compared against a cold-hydrated record's saved text; the
   * hash was computed over that exact text at generation time, so an
   * unedited reopen always matches). Absent alongside an absent sub-report,
   * and absent entirely on a report generated before this field existed
   * (never treated as stale — there is nothing to compare against).
   */
  sourceTextHash?: { resume?: number; coverLetter?: number };
}

/**
 * Cheap, stable, non-cryptographic string hash (djb2) — used only to detect
 * "this text changed since validation," never for security. Exported so every
 * comparison (live edit-diff, cold-entry hydration, a future re-check) uses
 * the exact same algorithm the report was hashed with.
 */
export function hashText(text: string): number {
  let hash = 5381;
  for (let i = 0; i < text.length; i++) {
    hash = (hash * 33) ^ text.charCodeAt(i);
  }
  return hash >>> 0; // unsigned 32-bit
}

/**
 * Validate whichever of `resumeText`/`coverLetterText` is non-empty against
 * `sourceResume` + `jobAd`, in parallel. Returns `null` when neither text was
 * generated, or when every validation call failed — the caller then leaves
 * `qualityReport` off its save request (existing report on the record, if
 * any, survives untouched — see `AiGenerationSaveRequest.qualityReport`).
 */
export async function computeQualityReport(params: {
  sourceResume: string;
  jobAd: string;
  topRequirements: string[];
  targetLanguage: string;
  resumeText?: string;
  coverLetterText?: string;
}): Promise<QualityReport | null> {
  const { sourceResume, jobAd, topRequirements, targetLanguage, resumeText, coverLetterText } =
    params;
  if (!resumeText?.trim() && !coverLetterText?.trim()) return null;

  const check = async (
    generated: string,
    docKind: 'resume' | 'coverLetter'
  ): Promise<ContentReportPayload | undefined> => {
    try {
      return await getClient().resume.validateContent({
        generated,
        source: sourceResume,
        jobAd,
        topRequirements,
        targetLanguage,
        docKind,
      });
    } catch (err) {
      console.warn('[computeQualityReport] validation failed — no report for this doc', {
        docKind,
        error: errorDetail(err),
      });
      return undefined;
    }
  };

  const [resume, coverLetter] = await Promise.all([
    resumeText?.trim() ? check(resumeText, 'resume') : Promise.resolve(undefined),
    coverLetterText?.trim() ? check(coverLetterText, 'coverLetter') : Promise.resolve(undefined),
  ]);
  if (!resume && !coverLetter) return null;

  const sourceTextHash: { resume?: number; coverLetter?: number } = {};
  if (resume) sourceTextHash.resume = hashText(resumeText ?? '');
  if (coverLetter) sourceTextHash.coverLetter = hashText(coverLetterText ?? '');

  return {
    schemaVersion: 1,
    pipeline: 'fast',
    generatedAt: Date.now(),
    resume,
    coverLetter,
    sourceTextHash,
  };
}

/**
 * Merge a freshly re-checked sub-report into an existing wrapper — replaces
 * only `docKind`'s payload + hash; the other doc (if any) and `generatedAt`
 * are left untouched, so a résumé-only re-check never disturbs a cover-letter
 * report sitting alongside it. Powers the quality panel's "Re-check" action,
 * which also clears staleness (the fresh hash matches the just-validated text).
 */
export function mergeRecheckedReport(
  existing: QualityReport | null,
  docKind: 'resume' | 'coverLetter',
  payload: ContentReportPayload,
  currentText: string
): QualityReport {
  const base: QualityReport = existing ?? {
    schemaVersion: 1,
    pipeline: 'fast',
    generatedAt: Date.now(),
  };
  const sourceTextHash: { resume?: number; coverLetter?: number } = {
    ...base.sourceTextHash,
    [docKind]: hashText(currentText),
  };
  return docKind === 'resume'
    ? { ...base, resume: payload, sourceTextHash }
    : { ...base, coverLetter: payload, sourceTextHash };
}

/** `undefined` (omit the field) for a null report — matches the save contract's
 *  "absent leaves the existing report untouched" semantics. */
export function serializeQualityReport(report: QualityReport | null): string | undefined {
  return report ? JSON.stringify(report) : undefined;
}

/**
 * Parse a persisted `AiGenerationRecord.qualityReport` string back into a
 * {@link QualityReport} for cold-entry hydration. Absent/empty/the Rust-side
 * `'{}'` placeholder, or anything that doesn't parse as an object, all become
 * `null` — never throws.
 */
export function parseQualityReport(raw: string | undefined): QualityReport | null {
  if (!raw || raw === EMPTY_REPORT_PLACEHOLDER) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    return parsed && typeof parsed === 'object' ? (parsed as QualityReport) : null;
  } catch {
    return null;
  }
}
