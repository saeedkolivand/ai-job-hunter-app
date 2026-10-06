/**
 * Guards + normalizers for the read-tier replies: `agent.result`,
 * `agent.call.result`, `settings.result`, `document.result`. Refusals may carry
 * `detail` and (on a throttle) `retryAfterMs`; those are rebuilt, never passed
 * through blindly.
 */

import type {
  ExtensionAgentCallResult,
  ExtensionAgentQueryResult,
  ExtensionDocumentExportResult,
  ExtensionSettingsResult,
} from '@ajh/shared/extension-protocol';

import { asRecord, isRefusalTail, withRefusalExtras } from './guards';

/**
 * Mirrors `ExtensionAgentQueryResultSchema`'s discriminated union: `ok:true`
 * requires a string `resource` + an OWN `data` property (Rust always emits
 * `data` on success — a payload missing it entirely is malformed, never a
 * silent `data: undefined`); `ok:false` requires a string `resource` + a
 * refusal tail.
 */
function isExtensionAgentQueryResult(v: unknown): v is ExtensionAgentQueryResult {
  const o = asRecord(v);
  if (o === null || typeof o.resource !== 'string') return false;
  if (o.ok === true) return Object.hasOwn(o, 'data');
  return o.ok === false && isRefusalTail(o);
}

export function normalizeAgentQueryResult(payload: unknown): ExtensionAgentQueryResult {
  if (!isExtensionAgentQueryResult(payload)) {
    return { ok: false, resource: '', error: 'The desktop app sent a malformed read result.' };
  }
  if (payload.ok) return { ok: true, resource: payload.resource, data: payload.data };
  return withRefusalExtras(
    { ok: false as const, resource: payload.resource, error: payload.error },
    payload
  );
}

/**
 * Mirrors `ExtensionAgentCallResultSchema`'s discriminated union on
 * `dispatched` (never `ok` — ADR-038 §5): `dispatched:true` requires string
 * `namespace`/`command` + an OWN `data` property; `dispatched:false` requires
 * string `namespace`/`command` + a refusal tail.
 */
function isExtensionAgentCallResult(v: unknown): v is ExtensionAgentCallResult {
  const o = asRecord(v);
  if (o === null || typeof o.namespace !== 'string' || typeof o.command !== 'string') return false;
  if (o.dispatched === true) return Object.hasOwn(o, 'data');
  return o.dispatched === false && isRefusalTail(o);
}

export function normalizeAgentCallResult(payload: unknown): ExtensionAgentCallResult {
  if (!isExtensionAgentCallResult(payload)) {
    return {
      dispatched: false,
      namespace: '',
      command: '',
      error: 'The desktop app sent a malformed call result.',
    };
  }
  if (payload.dispatched) {
    return {
      dispatched: true,
      namespace: payload.namespace,
      command: payload.command,
      data: payload.data,
    };
  }
  return withRefusalExtras(
    {
      dispatched: false as const,
      namespace: payload.namespace,
      command: payload.command,
      error: payload.error,
    },
    payload
  );
}

/**
 * Mirrors `ExtensionSettingsResultSchema`'s discriminated union: `ok:true`
 * requires a `settings` object whose first three fields are booleans;
 * `ok:false` requires a string `error`. The fourth field
 * (`saveAnswersOnSubmit`) may be ABSENT — a protocol-v2 desktop from before it
 * existed answers with only the first three keys — `normalizeSettingsResult`
 * fills it in as `false`. A PRESENT but wrong-typed fourth key still fails.
 */
function isExtensionSettingsResult(v: unknown): v is ExtensionSettingsResult {
  const o = asRecord(v);
  if (o === null) return false;
  if (o.ok === true) {
    const s = asRecord(o.settings);
    return (
      s !== null &&
      typeof s.autofill === 'boolean' &&
      typeof s.aiAssist === 'boolean' &&
      typeof s.autotrack === 'boolean' &&
      (s.saveAnswersOnSubmit === undefined || typeof s.saveAnswersOnSubmit === 'boolean')
    );
  }
  if (o.ok === false) return typeof o.error === 'string';
  return false;
}

/** `saveAnswersOnSubmit` is normalized to `false` when the desktop omitted it
 *  so Settings/Prep degrade to "the feature is off" rather than "unknown". */
export function normalizeSettingsResult(payload: unknown): ExtensionSettingsResult {
  if (!isExtensionSettingsResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed settings result.' };
  }
  return payload.ok
    ? {
        ok: true,
        settings: {
          ...payload.settings,
          saveAnswersOnSubmit: payload.settings.saveAnswersOnSubmit ?? false,
        },
      }
    : { ok: false, error: payload.error };
}

/**
 * Mirrors `ExtensionDocumentExportResultSchema`'s discriminated union: `ok:true`
 * requires a string `data` + the literal `dataEncoding: 'base64'` + string
 * `mimeType`/`filename` + numeric `byteLength` + the echoed
 * `kind`/`format`/`templateId`; `ok:false` is a refusal tail. This checks
 * `dataEncoding === 'base64'` ONLY — it never decodes `data` (that is the
 * caller's job).
 */
function isExtensionDocumentExportResult(v: unknown): v is ExtensionDocumentExportResult {
  const o = asRecord(v);
  if (o === null) return false;
  if (o.ok === true) {
    return (
      typeof o.data === 'string' &&
      o.dataEncoding === 'base64' &&
      typeof o.mimeType === 'string' &&
      typeof o.filename === 'string' &&
      typeof o.byteLength === 'number' &&
      (o.kind === 'resume' || o.kind === 'cover-letter') &&
      (o.format === 'pdf' || o.format === 'docx' || o.format === 'txt') &&
      typeof o.templateId === 'string'
    );
  }
  return o.ok === false && isRefusalTail(o);
}

export function normalizeDocumentExportResult(payload: unknown): ExtensionDocumentExportResult {
  if (!isExtensionDocumentExportResult(payload)) {
    return { ok: false, error: 'The desktop app sent a malformed document-export result.' };
  }
  if (payload.ok) {
    return {
      ok: true,
      data: payload.data,
      dataEncoding: 'base64',
      mimeType: payload.mimeType,
      filename: payload.filename,
      byteLength: payload.byteLength,
      kind: payload.kind,
      format: payload.format,
      templateId: payload.templateId,
    };
  }
  return withRefusalExtras({ ok: false as const, error: payload.error }, payload);
}
