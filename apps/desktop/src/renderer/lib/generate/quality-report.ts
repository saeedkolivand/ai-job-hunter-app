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

  return { schemaVersion: 1, pipeline: 'fast', generatedAt: Date.now(), resume, coverLetter };
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
