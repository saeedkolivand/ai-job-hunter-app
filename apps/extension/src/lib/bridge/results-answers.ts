/**
 * Guards + normalizers for the answer/fit replies: `answers.result`,
 * `answers.suggest.result`, `match.result`, `answer.assist.result` and the
 * streaming `assist.chunk` payload. Their errors are surfaced to the user, so
 * every malformed-reply fallback is `ok:false` + a plain `error`.
 */

import type {
  ExtensionAnswerAssistResult,
  ExtensionAnswersSaveResult,
  ExtensionAnswersSuggestResult,
  ExtensionAnswerSuggestion,
  ExtensionAssistChunkPayload,
  ExtensionMatchLiveResult,
} from '@ajh/shared/extension-protocol';

import { asRecord, optStr, pickDefined } from './guards';

/**
 * Mirrors `ExtensionAnswersSaveResultSchema`'s discriminated union: `ok:true`
 * requires a string `applicationId` + numeric `saved`/`skipped` (title/company
 * optional strings); `ok:false` requires a string `error`.
 */
function isExtensionAnswersSaveResult(v: unknown): v is ExtensionAnswersSaveResult {
  const o = asRecord(v);
  if (o === null) return false;
  if (o.ok === true) {
    return (
      typeof o.applicationId === 'string' &&
      typeof o.saved === 'number' &&
      typeof o.skipped === 'number' &&
      optStr(o.title) &&
      optStr(o.company)
    );
  }
  if (o.ok === false) return typeof o.error === 'string';
  return false;
}

export function normalizeAnswersSaveResult(payload: unknown): ExtensionAnswersSaveResult {
  if (!isExtensionAnswersSaveResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed answers-save result.' };
  }
  if (!payload.ok) return { ok: false, error: payload.error };
  return {
    ok: true,
    applicationId: payload.applicationId,
    saved: payload.saved,
    skipped: payload.skipped,
    ...pickDefined(payload, ['title', 'company'] as const),
  };
}

/** One `answers.suggest` suggestion entry — mirrors `ExtensionAnswerSuggestionSchema`. */
function isExtensionAnswerSuggestion(v: unknown): v is ExtensionAnswerSuggestion {
  const o = asRecord(v);
  return (
    o !== null &&
    typeof o.question === 'string' &&
    typeof o.answer === 'string' &&
    optStr(o.sourceCompany) &&
    optStr(o.sourceTitle) &&
    typeof o.sourceQuestion === 'string' &&
    typeof o.score === 'number' &&
    typeof o.salary === 'boolean'
  );
}

/**
 * Mirrors `ExtensionAnswersSuggestResultSchema`'s discriminated union: `ok:true`
 * requires a `suggestions` array of well-formed entries; `ok:false` requires a
 * string `error`.
 */
function isExtensionAnswersSuggestResult(v: unknown): v is ExtensionAnswersSuggestResult {
  const o = asRecord(v);
  if (o === null) return false;
  if (o.ok === true) {
    return Array.isArray(o.suggestions) && o.suggestions.every(isExtensionAnswerSuggestion);
  }
  if (o.ok === false) return typeof o.error === 'string';
  return false;
}

export function normalizeAnswersSuggestResult(payload: unknown): ExtensionAnswersSuggestResult {
  if (!isExtensionAnswersSuggestResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed suggestions result.' };
  }
  if (!payload.ok) return { ok: false, error: payload.error };
  return {
    ok: true,
    suggestions: payload.suggestions.map((s) => ({
      question: s.question,
      answer: s.answer,
      sourceQuestion: s.sourceQuestion,
      score: s.score,
      salary: s.salary,
      ...pickDefined(s, ['sourceCompany', 'sourceTitle'] as const),
    })),
  };
}

/**
 * Mirrors `ExtensionMatchLiveResultSchema`'s discriminated union: `ok:true`
 * requires numeric `combined`/`ats`, a string-array `gaps`, a string
 * `resumeName`, and a `scoreSource` literal (the optional `semantic` is
 * wire-reserved — never sent by the current desktop, but validated as
 * numeric-or-absent so a future desktop's value round-trips); `ok:false`
 * requires a string `error`. `salary` (additive/optional) is validated only
 * when PRESENT: a `posting` string, with an optional string `expectation` —
 * never a computed/verdict field.
 */
function isExtensionMatchLiveResult(v: unknown): v is ExtensionMatchLiveResult {
  const o = asRecord(v);
  if (o === null) return false;
  if (o.ok === true) {
    if (!(
      typeof o.combined === 'number' &&
      typeof o.ats === 'number' &&
      (o.semantic === undefined || typeof o.semantic === 'number') &&
      Array.isArray(o.gaps) &&
      o.gaps.every((g) => typeof g === 'string') &&
      typeof o.resumeName === 'string' &&
      (o.scoreSource === 'keyword' || o.scoreSource === 'combined')
    )) {
      return false;
    }
    if (o.salary === undefined) return true;
    const s = asRecord(o.salary);
    return s !== null && typeof s.posting === 'string' && optStr(s.expectation);
  }
  if (o.ok === false) return typeof o.error === 'string';
  return false;
}

export function normalizeMatchLiveResult(payload: unknown): ExtensionMatchLiveResult {
  if (!isExtensionMatchLiveResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed match result.' };
  }
  if (!payload.ok) return { ok: false, error: payload.error };
  const out: ExtensionMatchLiveResult = {
    ok: true,
    combined: payload.combined,
    ats: payload.ats,
    gaps: payload.gaps,
    resumeName: payload.resumeName,
    scoreSource: payload.scoreSource,
    ...pickDefined(payload, ['semantic'] as const),
  };
  if (payload.salary !== undefined) {
    out.salary = {
      posting: payload.salary.posting,
      ...pickDefined(payload.salary, ['expectation'] as const),
    };
  }
  return out;
}

/**
 * Mirrors `ExtensionAnswerAssistResultSchema`'s discriminated union: `ok:true`
 * requires string `question`/`draft` + a `sourced` object whose fields (all
 * optional) must be booleans when present; `ok:false` requires a string `error`.
 */
function isExtensionAnswerAssistResult(v: unknown): v is ExtensionAnswerAssistResult {
  const o = asRecord(v);
  if (o === null) return false;
  if (o.ok === true) {
    if (typeof o.question !== 'string' || typeof o.draft !== 'string') return false;
    const s = asRecord(o.sourced);
    const optBool = (x: unknown): boolean => x === undefined || typeof x === 'boolean';
    return s !== null && optBool(s.web) && optBool(s.brief) && optBool(s.salary);
  }
  if (o.ok === false) return typeof o.error === 'string';
  return false;
}

export function normalizeAnswerAssistResult(payload: unknown): ExtensionAnswerAssistResult {
  if (!isExtensionAnswerAssistResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed answer-assist result.' };
  }
  if (!payload.ok) return { ok: false, error: payload.error };
  return { ok: true, question: payload.question, draft: payload.draft, sourced: payload.sourced };
}

/** Mirrors `ExtensionAssistChunkPayloadSchema`: `delta` must be a string. */
export function isAssistChunkPayload(v: unknown): v is ExtensionAssistChunkPayload {
  return typeof asRecord(v)?.delta === 'string';
}
