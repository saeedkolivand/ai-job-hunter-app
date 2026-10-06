// The read-tier verbs (agent.query / agent.call / settings.get/set) and
// document.export mirror the matchLive suite: a deliberate action whose
// well-formed refusal (`ok:false` / `dispatched:false`) is NEVER folded away.

import { describe, expect, it, vi } from 'vitest';

import {
  connectedClient,
  itMalformed,
  itRoundTrips,
  sendAndAwaitFrame,
  setupFakeWebSocket,
  T,
  type VerbCase,
} from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

const fake = setupFakeWebSocket();

describe('BridgeClient – agentQuery', () => {
  const verb = (params?: Record<string, unknown>): VerbCase => ({
    call: (c) => c.agentQuery('job', params),
    sentType: T.agentQuery,
    sentPayload: { resource: 'job', ...params },
    replyType: T.agentResult,
  });
  const throttle = {
    ok: false,
    resource: 'job',
    error: 'rate_limited',
    detail: 'Too many requests — try again shortly.',
    retryAfterMs: 500,
  };

  itRoundTrips(fake, verb({ url: 'https://example.com/job/1' }), [
    [
      'round-trips a success result into the resolved payload',
      { ok: true, resource: 'job', data: { title: 'Engineer', company: 'Acme' } },
    ],
  ]);
  itRoundTrips(fake, verb(), [
    [
      'round-trips a desktop-side refusal (ok:false + error) — never rejects',
      { ok: false, resource: 'job', error: 'Autofill is off.' },
    ],
    ['round-trips a throttle refusal carrying detail + retryAfterMs', throttle],
  ]);
  itMalformed(
    fake,
    verb(),
    [
      ['resolves with a malformed error (never throws) when the payload is bad', { ok: true }],
      // `resource` present, `ok: true`, but no `data` key at all — Rust always
      // emits `data` on success, so this must never be a valid `data: undefined`.
      [
        'resolves with a malformed error (never throws) when an ok:true payload has no data property',
        { ok: true, resource: 'job' },
      ],
    ],
    'ok'
  );

  it('spreads params before resource so a colliding params.resource key can never override it', async () => {
    const { client, socket } = await connectedClient(fake);
    const { frame } = await sendAndAwaitFrame(socket, () =>
      client.agentQuery('job', { resource: 'other', url: 'https://example.com/job/1' })
    );
    expect(frame.payload).toEqual({ resource: 'job', url: 'https://example.com/job/1' });
    client.dispose();
  });
});

describe('BridgeClient – agentCall', () => {
  const verb = (command: string, args?: unknown): VerbCase => {
    const [namespace, name] = command.split(':');
    return {
      call: (c) => c.agentCall(command, args),
      sentType: T.agentCall,
      sentPayload: { namespace, command: name, input: args ?? {} },
      replyType: T.agentCallResult,
    };
  };
  const target = { namespace: 'documents', command: 'list' };

  itRoundTrips(fake, verb('documents:list', { limit: 10 }), [
    [
      'sends command + args and round-trips a dispatched:true result',
      { dispatched: true, ...target, data: { items: [] } },
    ],
  ]);
  itRoundTrips(fake, verb('documents:delete'), [
    [
      'round-trips a dispatched:false refusal (e.g. a non-Read effect) — never rejects',
      {
        dispatched: false,
        namespace: 'documents',
        command: 'delete',
        error: 'this tier only dispatches Read commands',
      },
    ],
  ]);
  itRoundTrips(fake, verb('documents:list'), [
    [
      'round-trips a dispatched:false throttle refusal carrying retryAfterMs',
      { dispatched: false, ...target, error: 'rate_limited', retryAfterMs: 500 },
    ],
  ]);
  // `dispatched: true` but no `data` key at all — Rust always emits `data` on success.
  itMalformed(
    fake,
    verb('documents:list'),
    [
      [
        'resolves with a malformed error (never throws) when a dispatched:true payload has no data property',
        { dispatched: true, ...target },
      ],
    ],
    'dispatched'
  );
});

describe('BridgeClient – settingsGet / settingsSet', () => {
  const get: VerbCase = {
    call: (c) => c.settingsGet(),
    sentType: T.settingsGet,
    sentPayload: {},
    replyType: T.settingsResult,
  };
  const set: VerbCase = {
    call: (c) => c.settingsSet('autofill', true),
    sentType: T.settingsSet,
    sentPayload: { key: 'autofill', enabled: true },
    replyType: T.settingsResult,
  };
  const flags = { autofill: true, aiAssist: false, autotrack: false };
  const current = { ok: true, settings: { ...flags, saveAnswersOnSubmit: false } };

  itRoundTrips(fake, get, [
    ['settingsGet sends an empty payload and round-trips the current values', current],
    // A pre-PR4 desktop's three-key reply — the guard must accept it and
    // normalize the missing fourth key to `false` (the safe "off" default) so
    // Settings/Prep degrade to "the feature is off", not "unknown".
    [
      'normalizes a settings.result missing saveAnswersOnSubmit (a pre-PR4/protocol-v2 desktop) to false rather than rejecting the whole reply',
      { ok: true, settings: flags },
      current,
    ],
    [
      'still treats a settings.result with a PRESENT but wrong-typed fourth key as malformed',
      { ok: true, settings: { ...flags, saveAnswersOnSubmit: 'no' } },
      { ok: false, error: 'The desktop app sent a malformed settings result.' },
    ],
  ]);
  itRoundTrips(fake, set, [
    ['settingsSet sends the key/enabled and round-trips the new values', current],
    [
      'settingsSet round-trips a refusal (e.g. a malformed key/value) — never rejects',
      { ok: false, error: 'invalid_settings_request' },
    ],
  ]);

  it('settles an in-flight settingsGet instead of leaving it hanging when dispose() is called', async () => {
    const { client, socket } = await connectedClient(fake);
    const { promise } = await sendAndAwaitFrame(socket, () => client.settingsGet());
    client.dispose();
    expect(await promise).toEqual({ ok: false, error: 'Bridge client disposed.' });
  });
});

describe('BridgeClient – documentExport', () => {
  const request = {
    source: { kind: 'generation' as const, url: 'https://example.com/job/1' },
    kind: 'resume' as const,
    format: 'pdf' as const,
    templateId: 'classic',
  };
  const verb: VerbCase = {
    call: (c) => c.documentExport(request),
    sentType: T.documentExport,
    sentPayload: request, // forwarded verbatim
    replyType: T.documentResult,
  };
  const exported = {
    ok: true,
    data: 'JVBERi0xLjQK',
    dataEncoding: 'base64',
    mimeType: 'application/pdf',
    filename: 'resume.pdf',
    byteLength: 9,
    kind: 'resume',
    format: 'pdf',
    templateId: 'classic',
  };

  itRoundTrips(fake, verb, [
    ['sends the request payload verbatim and round-trips a success result', exported],
    [
      'round-trips a desktop-side refusal (ok:false + error) — never rejects',
      { ok: false, error: 'export_failed', detail: 'no text' },
    ],
    [
      'round-trips a throttle refusal carrying retryAfterMs',
      { ok: false, error: 'rate_limited', retryAfterMs: 5_000 },
    ],
  ]);
  itMalformed(
    fake,
    verb,
    [
      [
        'resolves with a malformed error (never throws) when dataEncoding is not base64',
        { ...exported, data: 'abc', dataEncoding: 'utf8', byteLength: 3 },
      ],
    ],
    'ok'
  );

  it('settles with a refusal on dispose (failAllPending covers this verb too)', async () => {
    const { client, socket } = await connectedClient(fake);
    const { promise } = await sendAndAwaitFrame(socket, () => client.documentExport(request));
    client.dispose();
    expect(await promise).toEqual({ ok: false, error: 'Bridge client disposed.' });
  });
});
