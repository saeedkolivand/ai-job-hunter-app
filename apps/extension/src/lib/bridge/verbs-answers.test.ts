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

const URL_9 = 'https://jobs.example.com/posting/9';
const QUESTION = 'Why this role?';

describe('BridgeClient – saveAnswers', () => {
  const answers = [{ question: QUESTION, answer: 'Because I love it.' }];
  const verb: VerbCase = {
    call: (c) => c.saveAnswers(URL_9, answers),
    sentType: T.answersSave,
    sentPayload: { url: URL_9, answers },
    replyType: T.answersResult,
  };
  const refusal = {
    ok: false,
    error: "couldn't find a saved job for this page — import it first",
  };

  itRoundTrips(fake, verb, [
    [
      'round-trips a success result (incl. title/company) into the resolved payload',
      {
        ok: true,
        applicationId: 'app-1',
        saved: 1,
        skipped: 0,
        title: 'Backend Engineer',
        company: 'Acme',
      },
    ],
    [
      'round-trips a success result WITHOUT title/company (both optional)',
      { ok: true, applicationId: 'app-1', saved: 0, skipped: 0 },
    ],
    [
      'round-trips a desktop-side refusal (ok:false + error) into the resolved payload — never rejects',
      refusal,
    ],
  ]);
  // saved must be a number; a string breaks the guard.
  itMalformed(
    fake,
    verb,
    [
      [
        'resolves with a malformed error (never throws) when the payload is bad',
        { ok: true, applicationId: 'app-1', saved: 'one', skipped: 0 },
      ],
    ],
    'ok'
  );

  it.each([
    ['sends auto:true on the wire when the auto param is true (PR4)', true, { auto: true }],
    [
      'omits auto entirely when the param is left at its default (byte-identical to before PR4)',
      undefined,
      {},
    ],
  ])('%s', async (_title, auto, extra) => {
    const { client, socket } = await connectedClient(fake);
    const { frame } = await sendAndAwaitFrame(socket, () =>
      client.saveAnswers(URL_9, answers, auto)
    );
    expect(frame.payload).toStrictEqual({ url: URL_9, answers, ...extra });
    client.dispose();
  });
});

describe('BridgeClient – suggestAnswers', () => {
  const verb: VerbCase = {
    call: (c) => c.suggestAnswers([QUESTION]),
    sentType: T.answersSuggest,
    sentPayload: { questions: [QUESTION] },
    replyType: T.answersSuggestResult,
  };
  const suggestion = {
    question: QUESTION,
    answer: 'Because I love it.',
    sourceCompany: 'Acme',
    sourceTitle: 'Backend Engineer',
    sourceQuestion: QUESTION,
    score: 0.8,
    salary: false,
  };

  itRoundTrips(fake, verb, [
    [
      'round-trips a success result with a full suggestion into the resolved payload',
      { ok: true, suggestions: [suggestion] },
    ],
    ['round-trips a success result with an empty suggestions array', { ok: true, suggestions: [] }],
    [
      'round-trips a desktop-side refusal (ok:false + error) into the resolved payload — never rejects',
      { ok: false, error: 'Autofill is off.' },
    ],
  ]);
  // score must be a number; a string breaks the guard.
  itMalformed(
    fake,
    verb,
    [
      [
        'resolves with a malformed error (never throws) when the payload is bad',
        {
          ok: true,
          suggestions: [{ question: QUESTION, answer: 'x', score: 'high', salary: false }],
        },
      ],
    ],
    'ok'
  );
});

describe('BridgeClient – matchLive', () => {
  const request = { url: 'https://jobs.example.com/posting/1', html: '<html>job</html>' };
  const verb: VerbCase = {
    call: (c) => c.matchLive(request),
    sentType: T.matchLive,
    sentPayload: request,
    replyType: T.matchResult,
  };
  const match = {
    ok: true,
    combined: 72,
    ats: 60,
    gaps: [] as string[],
    resumeName: 'My Resume',
    scoreSource: 'keyword',
  };
  const NO_RESUME = 'Add a resume in AI Job Hunter first, then try Check fit again.';

  itRoundTrips(fake, verb, [
    [
      'round-trips a success result into the resolved payload',
      { ...match, gaps: ['kubernetes', 'terraform'] },
    ],
    [
      'round-trips a desktop-side refusal (ok:false + error) into the resolved payload — never rejects',
      { ok: false, error: NO_RESUME },
    ],
    [
      'round-trips the optional PR3 salary object',
      { ...match, salary: { posting: '€70,000–€90,000', expectation: '€80,000' } },
    ],
    // No `salary` key in → none out (an older desktop, additive field).
    [
      'leaves salary undefined when the desktop omitted it (an older desktop, additive field)',
      match,
    ],
  ]);
  // combined must be a number; a string breaks the guard.
  itMalformed(
    fake,
    verb,
    [
      [
        'resolves with a malformed error (never throws) when the payload is bad',
        { ...match, combined: 'high' },
      ],
    ],
    'ok'
  );
});
