import { describe, expect, it } from 'vitest';

import {
  ExtensionAgentCallRequestSchema,
  ExtensionAgentCallResultSchema,
  ExtensionAgentQueryRequestSchema,
  ExtensionAgentQueryResultSchema,
  ExtensionEnvelopeSchema,
} from '../extension-protocol.js';
import { EXTENSION_MESSAGE_TYPES } from '../extension-protocol-constants.js';

// ---------------------------------------------------------------------------
// ExtensionAgentQueryRequestSchema / ExtensionAgentQueryResultSchema (PR1 — extension read tier)
// ---------------------------------------------------------------------------

describe('ExtensionAgentQueryRequestSchema', () => {
  it('accepts a minimal request (resource only)', () => {
    expect(() => ExtensionAgentQueryRequestSchema.parse({ resource: 'job' })).not.toThrow();
  });

  it('accepts a request with resource-specific fields spread at the top level', () => {
    expect(() =>
      ExtensionAgentQueryRequestSchema.parse({
        resource: 'job',
        url: 'https://example.com/job/123',
      })
    ).not.toThrow();
  });

  it('rejects an empty resource', () => {
    expect(() => ExtensionAgentQueryRequestSchema.parse({ resource: '' })).toThrow();
  });

  it('rejects a request with no resource field', () => {
    expect(() => ExtensionAgentQueryRequestSchema.parse({})).toThrow();
  });
});

describe('ExtensionAgentQueryResultSchema', () => {
  it('round-trips a success payload', () => {
    const payload = { ok: true, resource: 'job', data: { title: 'Engineer' } };
    expect(ExtensionAgentQueryResultSchema.parse(payload)).toEqual(payload);
  });

  it('accepts a user-facing failure payload naming the resource', () => {
    expect(() =>
      ExtensionAgentQueryResultSchema.parse({
        ok: false,
        resource: 'job',
        error: 'Assisted autofill is off.',
      })
    ).not.toThrow();
  });

  it('round-trips a throttle refusal carrying detail + retryAfterMs', () => {
    const payload = {
      ok: false,
      resource: 'job',
      error: 'rate_limited',
      detail: 'Too many requests — try again shortly.',
      retryAfterMs: 500,
    };
    expect(ExtensionAgentQueryResultSchema.parse(payload)).toEqual(payload);
  });

  it('rejects a missing ok field', () => {
    expect(() => ExtensionAgentQueryResultSchema.parse({ resource: 'job' })).toThrow();
  });

  it('rejects an incomplete ok:true payload missing resource', () => {
    expect(() => ExtensionAgentQueryResultSchema.parse({ ok: true, data: {} })).toThrow();
  });

  it('carries agent.query / agent.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.agentQuery,
        reqId: 'req-014',
        payload: { resource: 'job', url: 'https://example.com/job/123' },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.agentResult,
        reqId: 'req-015',
        payload: { ok: true, resource: 'job', data: {} },
      })
    ).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// ExtensionAgentCallRequestSchema / ExtensionAgentCallResultSchema (PR1 — extension read tier)
// ---------------------------------------------------------------------------

describe('ExtensionAgentCallRequestSchema', () => {
  it('accepts a minimal request (namespace + command only)', () => {
    expect(() =>
      ExtensionAgentCallRequestSchema.parse({ namespace: 'documents', command: 'list' })
    ).not.toThrow();
  });

  it('accepts a request with input', () => {
    expect(() =>
      ExtensionAgentCallRequestSchema.parse({
        namespace: 'documents',
        command: 'list',
        input: { limit: 10 },
      })
    ).not.toThrow();
  });

  it('rejects an empty command', () => {
    expect(() =>
      ExtensionAgentCallRequestSchema.parse({ namespace: 'documents', command: '' })
    ).toThrow();
  });

  it('rejects a request with no command field', () => {
    expect(() => ExtensionAgentCallRequestSchema.parse({ namespace: 'documents' })).toThrow();
  });

  it('rejects a request with no namespace field', () => {
    expect(() => ExtensionAgentCallRequestSchema.parse({ command: 'list' })).toThrow();
  });
});

describe('ExtensionAgentCallResultSchema', () => {
  it('round-trips a dispatched:true payload', () => {
    const payload = {
      dispatched: true,
      namespace: 'documents',
      command: 'list',
      data: { items: [] },
    };
    expect(ExtensionAgentCallResultSchema.parse(payload)).toEqual(payload);
  });

  it('round-trips a dispatched:false payload with an optional detail', () => {
    const payload = {
      dispatched: false,
      namespace: 'documents',
      command: 'delete',
      error: 'this tier only dispatches Read commands',
      detail: 'documents:delete is Irreversible',
    };
    expect(ExtensionAgentCallResultSchema.parse(payload)).toEqual(payload);
  });

  it('round-trips a dispatched:false throttle refusal carrying retryAfterMs', () => {
    const payload = {
      dispatched: false,
      namespace: 'documents',
      command: 'list',
      error: 'rate_limited',
      retryAfterMs: 500,
    };
    expect(ExtensionAgentCallResultSchema.parse(payload)).toEqual(payload);
  });

  it('rejects a missing dispatched field', () => {
    expect(() =>
      ExtensionAgentCallResultSchema.parse({ namespace: 'documents', command: 'list' })
    ).toThrow();
  });

  it('rejects a contradictory dispatched:false payload carrying success fields but no error', () => {
    expect(() =>
      ExtensionAgentCallResultSchema.parse({
        dispatched: false,
        namespace: 'documents',
        command: 'list',
        data: {},
      })
    ).toThrow();
  });

  it('carries agent.call / agent.call.result through a valid envelope', () => {
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.agentCall,
        reqId: 'req-016',
        payload: { namespace: 'documents', command: 'list' },
      })
    ).not.toThrow();
    expect(() =>
      ExtensionEnvelopeSchema.parse({
        type: EXTENSION_MESSAGE_TYPES.agentCallResult,
        reqId: 'req-017',
        payload: { dispatched: true, namespace: 'documents', command: 'list', data: {} },
      })
    ).not.toThrow();
  });
});
