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

/** Sentinel treated as "no report" here; Rust's `merge_quality_report` no-ops
 *  on it not by special case but because an empty object overlays no keys. */
const EMPTY_REPORT_PLACEHOLDER = '{}';

/**
 * One document's entry in the wrapper: the validator's verdict AND the hash of
 * the exact text that produced it, as one indivisible unit.
 */
export interface QualityReportSlot {
  /** This document's deterministic content report. */
  report: ContentReportPayload;
  /**
   * djb2 hash of the EXACT text `report` validated — same document text, same
   * moment. A reader compares `hashText(currentText)` against it to detect the
   * document has since diverged (a hand-edit) WITHOUT keeping a full second
   * copy of the text around: one mechanism covering both live staleness
   * (against the session's `resumeOut`/`coverOut`) and persisted staleness
   * (against a cold-hydrated record's saved text — the hash was computed over
   * that exact text at save time, so an unedited reopen always matches).
   */
  sourceTextHash: number;
}

/**
 * Wire shape persisted in `AiGenerationRecord.qualityReport` — an opaque JSON
 * string on the Rust side (the migration/merge logic never parses it), so this
 * wrapper is free to carry BOTH documents' reports plus provenance rather than
 * mirroring Rust's single-document `ContentReport` 1:1.
 *
 * **The slot shape is DESIGNED for the persistence merge.** Rust's
 * `merge_quality_report` overlays `incoming` onto `existing` per TOP-LEVEL key
 * (semantics-free — it never looks inside a value), so everything belonging to
 * one document has to live UNDER that document's key. v1's sibling
 * `sourceTextHash: { resume?, coverLetter? }` map broke exactly that rule and
 * was the bug: a résumé-only regeneration emits a map holding only `resume`
 * (it cannot know the letter's text), the merge replaced the whole map, and
 * the surviving `coverLetter` sub-report permanently lost its staleness anchor
 * — after which the badge showed a green "no issues" on hand-edited text the
 * report had never seen. With the hash inside the slot, replacing `resume`
 * replaces its hash atomically and `coverLetter` stays whole.
 */
export interface QualityReport {
  schemaVersion: 2;
  /** Which validation pipeline produced this — 'fast' is the only one today
   *  (the deterministic checks); reserved for a future slower/deeper pass. */
  pipeline: 'fast';
  generatedAt: number;
  resume?: QualityReportSlot;
  coverLetter?: QualityReportSlot;
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

  // A document this run did not validate contributes NO key at all — not an
  // `undefined` one. The merge overlays whatever keys this wrapper carries, so
  // a key it can't fill would overwrite the stored slot for the OTHER document.
  return {
    schemaVersion: 2,
    pipeline: 'fast',
    generatedAt: Date.now(),
    ...(resume ? { resume: { report: resume, sourceTextHash: hashText(resumeText ?? '') } } : {}),
    ...(coverLetter
      ? { coverLetter: { report: coverLetter, sourceTextHash: hashText(coverLetterText ?? '') } }
      : {}),
  };
}

/**
 * Merge a freshly re-checked sub-report into an existing wrapper — replaces
 * only `docKind`'s slot (payload AND hash, atomically); the other doc (if any)
 * and `generatedAt` are left untouched, so a résumé-only re-check never
 * disturbs a cover-letter report sitting alongside it. Powers the quality
 * panel's "Re-check" action, which also clears staleness (the fresh hash
 * matches the just-validated text).
 */
export function mergeRecheckedReport(
  existing: QualityReport | null,
  docKind: 'resume' | 'coverLetter',
  payload: ContentReportPayload,
  currentText: string
): QualityReport {
  const base: QualityReport = existing ?? {
    schemaVersion: 2,
    pipeline: 'fast',
    generatedAt: Date.now(),
  };
  const slot: QualityReportSlot = { report: payload, sourceTextHash: hashText(currentText) };
  return docKind === 'resume' ? { ...base, resume: slot } : { ...base, coverLetter: slot };
}

/** `undefined` (omit the field) for a null report — matches the save contract's
 *  "absent leaves the existing report untouched" semantics. */
export function serializeQualityReport(report: QualityReport | null): string | undefined {
  return report ? JSON.stringify(report) : undefined;
}

type ContentIssue = ContentReportPayload['issues'][number];
type ContentMetrics = ContentReportPayload['metrics'];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}

/** One issue entry's expected shape; a malformed entry is dropped rather than
 *  letting a bad `code`/`section`/`evidence` type reach the panel. */
function parseIssue(value: unknown): ContentIssue | null {
  if (!isRecord(value)) return null;
  const { severity, code, section, message, evidence } = value;
  if (severity !== 'critical' && severity !== 'warning') return null;
  if (typeof code !== 'string' || typeof message !== 'string') return null;
  if (!isNullableString(section) || !isNullableString(evidence)) return null;
  return { severity, code, section, message, evidence };
}

function parseMetrics(value: unknown): ContentMetrics | null {
  if (!isRecord(value)) return null;
  const num = (v: unknown): number => (typeof v === 'number' ? v : 0);
  return {
    keywordCoverage: typeof value.keywordCoverage === 'number' ? value.keywordCoverage : null,
    // null = "not measured" (uncomparable posting / no requirements / letter).
    // Coercing it to 0 here would resurrect "0 requirements covered" as a fact
    // on every reopen — the exact state the Option<u32> wire change removed.
    topRequirementHits:
      typeof value.topRequirementHits === 'number' ? value.topRequirementHits : null,
    // The denominator travels with the hits — dropping it here would strand a
    // reopened report with an uninterpretable bare count.
    topRequirementsMeasured:
      typeof value.topRequirementsMeasured === 'number' ? value.topRequirementsMeasured : null,
    duplicateRatio: num(value.duplicateRatio),
    rolesSource: num(value.rolesSource),
    rolesOutput: num(value.rolesOutput),
  };
}

/**
 * Validate one sub-report (`resume`/`coverLetter`) defensively — required so
 * a malformed persisted value (`{"issues":42}`, `{"issues":"abc"}`, a bare
 * boolean…) can never reach `QualityBadge`/`QualityReportPanel`, both of
 * which assume `issues` is a real array. Security finding M-1: an
 * unvalidated cast here crashed the whole app (only the root ErrorBoundary
 * caught it) every time that record's TailorFlow was opened. A malformed
 * sub-report drops to `undefined` — never thrown.
 */
function parseSubReport(value: unknown): ContentReportPayload | undefined {
  if (!isRecord(value) || !Array.isArray(value.issues)) return undefined;
  const metrics = parseMetrics(value.metrics);
  if (!metrics) return undefined;
  const issues = value.issues
    .map(parseIssue)
    .filter((issue): issue is ContentIssue => issue !== null);
  return { ok: typeof value.ok === 'boolean' ? value.ok : issues.length === 0, issues, metrics };
}

/**
 * One document slot: a valid sub-report AND its numeric hash. A slot missing
 * either half is dropped entirely rather than half-loaded — a report without
 * its hash is precisely the "green badge on text it never saw" state the slot
 * shape exists to prevent.
 */
function parseSlot(value: unknown): QualityReportSlot | undefined {
  if (!isRecord(value) || typeof value.sourceTextHash !== 'number') return undefined;
  const report = parseSubReport(value.report);
  return report ? { report, sourceTextHash: value.sourceTextHash } : undefined;
}

/**
 * Parse a persisted `AiGenerationRecord.qualityReport` string back into a
 * {@link QualityReport} for cold-entry hydration. Total: any input — absent,
 * empty, the Rust-side `'{}'` placeholder, invalid JSON, or a malformed shape
 * at any level — resolves to `null` (or a partial report with the malformed
 * slot dropped), never a thrown exception.
 *
 * `schemaVersion` gates the whole shape: anything other than `2` is treated as
 * absent rather than pattern-matched against its field names. That deliberately
 * includes **v1** (sub-reports at the top level beside a shared
 * `sourceTextHash` map): no migration is written for it because a v1 blob can
 * only exist on a dev machine that ran this feature branch before the slot
 * shape landed — never in a release — and "no report until the next
 * generation" is the correct degrade. Re-pairing a v1 map's hashes with its
 * sub-reports would also resurrect the very ambiguity (which hash belongs to
 * which report after a partial merge?) that v2 exists to remove.
 */
export function parseQualityReport(raw: string | undefined): QualityReport | null {
  if (!raw || raw === EMPTY_REPORT_PLACEHOLDER) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!isRecord(parsed) || parsed.schemaVersion !== 2) return null;

  const resume = parseSlot(parsed.resume);
  const coverLetter = parseSlot(parsed.coverLetter);

  return {
    schemaVersion: 2,
    pipeline: 'fast',
    generatedAt: typeof parsed.generatedAt === 'number' ? parsed.generatedAt : 0,
    ...(resume ? { resume } : {}),
    ...(coverLetter ? { coverLetter } : {}),
  };
}
