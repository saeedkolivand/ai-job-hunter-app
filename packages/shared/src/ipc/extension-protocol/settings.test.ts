import { describe, expect, it } from 'vitest';

import {
  ExtensionEnvelopeSchema,
  ExtensionSettingsGetRequestSchema,
  ExtensionSettingsResultSchema,
  ExtensionSettingsSetRequestSchema,
} from '../extension-protocol.js';
import { EXTENSION_MESSAGE_TYPES } from '../extension-protocol-constants.js';

// ---------------------------------------------------------------------------
// ExtensionSettingsGetRequestSchema / ExtensionSettingsSetRequestSchema / ExtensionSettingsResultSchema
// ---------------------------------------------------------------------------

describe('ExtensionSettingsGetRequestSchema', () => {
  it('accepts an empty request', () => {
    expect(() => ExtensionSettingsGetRequestSchema.parse({})).not.toThrow();
  });

  it('rejects a request with a surplus field', () => {
    expect(() => ExtensionSettingsGetRequestSchema.parse({ extra: true })).toThrow();
  });
});

describe('ExtensionSettingsSetRequestSchema', () => {
  it.each(['autofill', 'aiAssist', 'autotrack', 'saveAnswersOnSubmit'] as const)(
    'accepts a valid request for key %s',
    (key) => {
      expect(() => ExtensionSettingsSetRequestSchema.parse({ key, enabled: true })).not.toThrow();
    }
  );

  it('rejects an unknown key', () => {
    expect(() =>
      ExtensionSettingsSetRequestSchema.parse({ key: 'bogusKey', enabled: true })
    ).toThrow();
  });

  it('rejects a non-boolean enabled field', () => {
    expect(() =>
      ExtensionSettingsSetRequestSchema.parse({ key: 'autofill', enabled: 'yes' })
    ).toThrow();
  });

  it('rejects a request with no key field', () => {
    expect(() => ExtensionSettingsSetRequestSchema.parse({ enabled: true })).toThrow();
  });
});

describe('ExtensionSettingsResultSchema', () => {
  it('round-trips a success payload', () => {
    const payload = {
      ok: true,
      settings: { autofill: true, aiAssist: false, autotrack: false, saveAnswersOnSubmit: false },
    };
    expect(ExtensionSettingsResultSchema.parse(payload)).toEqual(payload);
  });

  it('accepts a user-facing failure payload (a malformed key/value)', () => {
    expect(() =>
      ExtensionSettingsResultSchema.parse({ ok: false, error: 'invalid_settings_request' })
    ).not.toThrow();
  });

  it('rejects an incomplete ok:true payload missing a settings key', () => {
    expect(() =>
      ExtensionSettingsResultSchema.parse({
        ok: true,
        settings: { autofill: true, aiAssist: false },
      })
    ).toThrow();
  });

  it('accepts an ok:true payload missing ONLY the fourth key (protocol-v2 back-compat) and normalizes it to false', () => {
    const result = ExtensionSettingsResultSchema.parse({
      ok: true,
      settings: { autofill: true, aiAssist: false, autotrack: false },
    });
    expect(result).toEqual({
      ok: true,
      settings: { autofill: true, aiAssist: false, autotrack: false, saveAnswersOnSubmit: false },
    });
  });

  it('still rejects a PRESENT but wrong-typed fourth key', () => {
    expect(() =>
      ExtensionSettingsResultSchema.parse({
        ok: true,
        settings: { autofill: true, aiAssist: false, autotrack: false, saveAnswersOnSubmit: 'no' },
      })
    ).toThrow();
  });

  it('rejects a contradictory ok:false payload carrying settings but no error', () => {
    expect(() =>
      ExtensionSettingsResultSchema.parse({
        ok: false,
        settings: { autofill: true, aiAssist: false, autotrack: false, saveAnswersOnSubmit: false },
      })
    ).toThrow();
  });

  it('carries settings.get / settings.set / settings.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.settingsGet,
        reqId: 'req-018',
        payload: {},
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.settingsSet,
        reqId: 'req-019',
        payload: { key: 'saveAnswersOnSubmit', enabled: true },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.settingsResult,
        reqId: 'req-020',
        payload: {
          ok: true,
          settings: {
            autofill: true,
            aiAssist: false,
            autotrack: false,
            saveAnswersOnSubmit: false,
          },
        },
      })
    ).not.toThrow();
  });
});
