import { describe, vi } from 'vitest';

import { itMalformed, itRoundTrips, setupFakeWebSocket, T, type VerbCase } from './test-support';

vi.mock('@wxt-dev/browser', () => import('./browser-mock'));

const fake = setupFakeWebSocket();

const URL_9 = 'https://jobs.example.com/posting/9';

describe('BridgeClient – getProfile (assisted autofill)', () => {
  // The outgoing frame is a profile.get with a null payload (authed by the session only).
  const verb: VerbCase = {
    call: (c) => c.getProfile(),
    sentType: T.profileGet,
    sentPayload: null,
    replyType: T.profileResult,
  };
  const MALFORMED = { error: 'The desktop app sent a malformed profile result.' };
  const link = (label: string, url: string) => ({ label, url });
  const dropped = { email: 'saeed@example.com', extraLinks: [] };

  itRoundTrips(fake, verb, [
    [
      'round-trips a profile.result reply into the resolved profile fields',
      {
        fullName: 'Saeed Kolivand',
        email: 'saeed@example.com',
        phone: '+31 6 1234 5678',
        linkedin: 'https://linkedin.com/in/saeed',
      },
    ],
    [
      'round-trips extraLinks (additive/optional field) into the resolved profile',
      {
        email: 'saeed@example.com',
        extraLinks: [
          link('Portfolio', 'https://saeed.dev'),
          link('Dribbble', 'https://dribbble.com/saeed'),
        ],
      },
    ],
    [
      'tolerates the absence of extraLinks (old desktop reply, additive field)',
      { email: 'saeed@example.com' },
    ],
    [
      'drops an extraLinks entry missing url, keeping the rest of the profile',
      { email: 'saeed@example.com', extraLinks: [{ label: 'Portfolio' }] },
      dropped,
    ],
    [
      'drops an extraLinks entry with a non-http(s) url, keeping the rest of the profile (never a payload-level malformed error)',
      { email: 'saeed@example.com', extraLinks: [link('Portfolio', 'javascript:alert(1)')] },
      dropped,
    ],
    [
      'filters a mixed valid/invalid extraLinks array down to only the valid entries',
      {
        extraLinks: [
          link('Portfolio', 'https://saeed.dev'),
          link('Bad Scheme', 'javascript:alert(1)'),
          { label: 'Missing Url' },
          link('Dribbble', 'https://dribbble.com/saeed'),
        ],
      },
      {
        extraLinks: [
          link('Portfolio', 'https://saeed.dev'),
          link('Dribbble', 'https://dribbble.com/saeed'),
        ],
      },
    ],
    [
      'resolves with the refusal error when autofill is opted out on the desktop',
      { error: 'Autofill is off.' },
    ],
    // Never throws: the bad payload resolves with ONLY the malformed error (no `email`).
    [
      'resolves with a malformed error (never throws) when extraLinks is not an array',
      { extraLinks: 'https://saeed.dev' },
      MALFORMED,
    ],
    [
      'resolves with a malformed error (never throws) when the payload is bad',
      { email: 42 },
      MALFORMED,
    ],
  ]);
});

describe('BridgeClient – checkApplied', () => {
  const verb = (url: string): VerbCase => ({
    call: (c) => c.checkApplied(url),
    sentType: T.appliedCheck,
    sentPayload: { url },
    replyType: T.appliedResult,
  });

  itRoundTrips(fake, verb(URL_9), [
    [
      'round-trips a found+applied result into the resolved payload',
      {
        found: true,
        applicationId: 'app-1',
        status: 'applied',
        title: 'Senior Rust Engineer',
        appliedAt: 1_718_000_000_000,
      },
    ],
    ['round-trips a not-found result', { found: false }],
  ]);
  // found must be a boolean; a string breaks the guard.
  itMalformed(
    fake,
    verb('https://jobs.example.com/posting/bad'),
    [['resolves with a malformed error (never throws) when the payload is bad', { found: 'yes' }]],
    'found'
  );
});

describe('BridgeClient – checkAppliedBatch', () => {
  const urls = ['https://jobs.example.com/1', 'https://jobs.example.com/2'];
  const verb: VerbCase = {
    call: (c) => c.checkAppliedBatch(urls),
    sentType: T.appliedCheckBatch,
    sentPayload: { urls },
    replyType: T.appliedBatchResult,
  };

  itRoundTrips(fake, verb, [
    [
      'round-trips a success result, preserving order',
      {
        ok: true,
        results: [
          { url: urls[0], found: true, status: 'saved' },
          { url: urls[1], found: false },
        ],
      },
    ],
    [
      'round-trips a desktop-side refusal (over-cap/throttle) — never rejects',
      { ok: false, error: 'too_many_urls', retryAfterMs: 1500 },
    ],
  ]);
  // `results` entries must have a boolean `found`; a string breaks the guard.
  itMalformed(
    fake,
    verb,
    [
      [
        'resolves with a malformed error (never throws) when the payload is bad',
        { ok: true, results: [{ url: urls[0], found: 'yes' }] },
      ],
    ],
    'ok'
  );
});

describe('BridgeClient – updateStatus', () => {
  const verb = (url: string): VerbCase => ({
    call: (c) => c.updateStatus(url),
    sentType: T.statusUpdate,
    sentPayload: { url, to: 'applied' },
    replyType: T.statusResult,
  });
  const refusal = { ok: false, error: "couldn't find a saved job for this page" };

  itRoundTrips(fake, verb(URL_9), [
    [
      'round-trips a success result into the resolved payload',
      { ok: true, applicationId: 'app-1', status: 'applied' },
    ],
    [
      'round-trips a desktop-side refusal (ok:false + error) into the resolved payload — never rejects',
      refusal,
    ],
  ]);
  // ok must be a boolean; a string breaks the guard.
  itMalformed(
    fake,
    verb('https://jobs.example.com/posting/bad'),
    [['resolves with a malformed error (never throws) when the payload is bad', { ok: 'yes' }]],
    'ok'
  );
});
