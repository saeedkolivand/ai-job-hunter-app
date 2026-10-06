/**
 * Guards + normalizers for the tracking-side replies: `import.result`,
 * `profile.result`, `applied.result`, `applied.batch.result`, `status.result`.
 * Each `normalize*` degrades a malformed payload to a plain error result —
 * never a throw, never the raw payload.
 */

import type {
  ExtensionAppliedBatchEntry,
  ExtensionAppliedCheckBatchResult,
  ExtensionAppliedCheckResult,
  ExtensionImportResult,
  ExtensionProfileResult,
  ExtensionStatusUpdateResult,
} from '@ajh/shared/extension-protocol';

import { asRecord, isRefusalTail, optStr, pickDefined, withRefusalExtras } from './guards';

/** Mirrors `ExtensionImportResultSchema`: every field optional; `matchScore` a number, `partial` a boolean. */
function isExtensionImportResult(v: unknown): v is ExtensionImportResult {
  const o = asRecord(v);
  return (
    o !== null &&
    optStr(o.applicationId) &&
    optStr(o.status) &&
    optStr(o.title) &&
    optStr(o.company) &&
    optStr(o.error) &&
    (o.matchScore === undefined || typeof o.matchScore === 'number') &&
    (o.partial === undefined || typeof o.partial === 'boolean')
  );
}

export function normalizeImportResult(payload: unknown): ExtensionImportResult {
  if (!isExtensionImportResult(payload)) {
    return { error: 'The desktop app sent a malformed import result.' };
  }
  // Rebuilt from only the known, defined keys — mirrors the old zod
  // `.safeParse(...).data`, which STRIPPED unknown keys.
  return pickDefined(payload, [
    'applicationId',
    'status',
    'title',
    'company',
    'matchScore',
    'error',
    'partial',
  ] as const);
}

/** True when `x` is a plain `{label: string, url: string}` link entry — `url`
 *  must additionally be `http(s)://`. This is defense-in-depth against a
 *  buggy/older desktop sending a malformed scheme (e.g. `javascript:`): the
 *  bridge's own auth/allowlist model doesn't cover payload content, and a URL
 *  is the one field here that gets set into a form's `value` and dispatched
 *  as an event, so it self-defends rather than trusting the server. */
function isLinkEntry(x: unknown): x is { label: string; url: string } {
  const o = asRecord(x);
  return (
    o !== null &&
    typeof o.label === 'string' &&
    typeof o.url === 'string' &&
    /^https?:\/\//i.test(o.url)
  );
}

/**
 * Mirrors `ExtensionProfileResultSchema`: every field is an optional string,
 * except `extraLinks` — an optional array, additive to the payload (an old
 * desktop's reply simply never carries the key). Only the array shape is
 * gated here; individual entries are dropped one-by-one in
 * {@link normalizeProfileResult} via {@link isLinkEntry} so one malformed
 * link (e.g. a `javascript:` scheme) never rejects the whole profile.
 */
function isExtensionProfileResult(v: unknown): v is ExtensionProfileResult {
  const o = asRecord(v);
  return (
    o !== null &&
    optStr(o.fullName) &&
    optStr(o.email) &&
    optStr(o.phone) &&
    optStr(o.location) &&
    optStr(o.linkedin) &&
    optStr(o.github) &&
    optStr(o.website) &&
    (o.extraLinks === undefined || Array.isArray(o.extraLinks)) &&
    optStr(o.error)
  );
}

export function normalizeProfileResult(payload: unknown): ExtensionProfileResult {
  if (!isExtensionProfileResult(payload)) {
    return { error: 'The desktop app sent a malformed profile result.' };
  }
  const out: ExtensionProfileResult = pickDefined(payload, [
    'fullName',
    'email',
    'phone',
    'location',
    'linkedin',
    'github',
    'website',
    'error',
  ] as const);
  // Filtered (not `.every`-gated) so a single malformed entry is dropped
  // rather than failing the entire profile.
  if (payload.extraLinks !== undefined) out.extraLinks = payload.extraLinks.filter(isLinkEntry);
  return out;
}

/** Mirrors `ExtensionAppliedCheckResultSchema`: `found` a boolean, `appliedAt` an optional epoch-ms number, the rest optional strings. */
function isExtensionAppliedCheckResult(v: unknown): v is ExtensionAppliedCheckResult {
  const o = asRecord(v);
  return (
    o !== null &&
    typeof o.found === 'boolean' &&
    optStr(o.applicationId) &&
    optStr(o.status) &&
    optStr(o.title) &&
    (o.appliedAt === undefined || typeof o.appliedAt === 'number') &&
    optStr(o.error)
  );
}

export function normalizeAppliedCheckResult(payload: unknown): ExtensionAppliedCheckResult {
  if (!isExtensionAppliedCheckResult(payload)) {
    return { found: false, error: 'The desktop app sent a malformed applied-check result.' };
  }
  return {
    found: payload.found,
    ...pickDefined(payload, ['applicationId', 'status', 'title', 'appliedAt', 'error'] as const),
  };
}

/** One entry of an `applied.batch.result` `results` array. */
function isExtensionAppliedBatchEntry(v: unknown): v is ExtensionAppliedBatchEntry {
  const o = asRecord(v);
  return (
    o !== null &&
    typeof o.url === 'string' &&
    typeof o.found === 'boolean' &&
    (o.status === undefined || typeof o.status === 'string')
  );
}

/**
 * Mirrors `ExtensionAppliedCheckBatchResultSchema`'s discriminated union:
 * `ok:true` requires a `results` array of {@link ExtensionAppliedBatchEntry};
 * `ok:false` is a refusal (`error` + optional `detail`/`retryAfterMs`).
 */
function isExtensionAppliedCheckBatchResult(v: unknown): v is ExtensionAppliedCheckBatchResult {
  const o = asRecord(v);
  if (o === null) return false;
  if (o.ok === true) {
    return Array.isArray(o.results) && o.results.every(isExtensionAppliedBatchEntry);
  }
  return o.ok === false && isRefusalTail(o);
}

/** This verb's errors are NOT rendered directly (the caller degrades a
 *  refusal to "no stamps"), but the fallback text still names the failure. */
export function normalizeAppliedCheckBatchResult(
  payload: unknown
): ExtensionAppliedCheckBatchResult {
  if (!isExtensionAppliedCheckBatchResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed applied-batch result.' };
  }
  if (payload.ok) {
    return {
      ok: true,
      results: payload.results.map((r) => ({
        url: r.url,
        found: r.found,
        ...pickDefined(r, ['status'] as const),
      })),
    };
  }
  return withRefusalExtras({ ok: false as const, error: payload.error }, payload);
}

/**
 * Mirrors `ExtensionStatusUpdateResultSchema`'s discriminated union: `ok:true`
 * requires a string `applicationId` + the literal `status: 'applied'`;
 * `ok:false` requires a string `error`. Success and failure never mix.
 */
function isExtensionStatusUpdateResult(v: unknown): v is ExtensionStatusUpdateResult {
  const o = asRecord(v);
  if (o === null) return false;
  if (o.ok === true) return typeof o.applicationId === 'string' && o.status === 'applied';
  if (o.ok === false) return typeof o.error === 'string';
  return false;
}

/** Unlike the applied-check fallback, this verb's errors are surfaced to the user. */
export function normalizeStatusUpdateResult(payload: unknown): ExtensionStatusUpdateResult {
  if (!isExtensionStatusUpdateResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed status-update result.' };
  }
  return payload.ok
    ? { ok: true, applicationId: payload.applicationId, status: payload.status }
    : { ok: false, error: payload.error };
}
