/** `answerFill` / `answerReplace` — per-row write-back, NEVER a different field. */

import { beforeEach, describe, expect, it } from 'vitest';

import { executeScriptMock, paired, resetMocks, scriptResults, send } from './test-support';

beforeEach(resetMocks);

const NOT_FOUND = 'Could not find this field — the page may have changed.';

describe.each([
  {
    kind: 'answerFill',
    file: 'answer-fill.js',
    request: {
      kind: 'answerFill',
      question: 'Why this role?',
      index: 0,
      count: 1,
      answer: 'Because I love it.',
    },
    args: ['Why this role?', 0, 1, 'Because I love it.', '__ajhRunAnswerFill'],
    malformed: 'Could not fill this field.',
  },
  {
    kind: 'answerReplace',
    file: 'answer-replace.js',
    request: {
      kind: 'answerReplace',
      question: 'Why this role?',
      index: 0,
      count: 1,
      text: 'A rewritten answer.',
      expectedValue: 'Because I like it.',
    },
    args: [
      'Why this role?',
      0,
      1,
      'A rewritten answer.',
      'Because I like it.',
      '__ajhRunAnswerReplace',
    ],
    malformed: 'Could not replace this field.',
  },
] as const)('$kind request', ({ kind, file, request, args, malformed }) => {
  beforeEach(() => paired());

  it('injects the runner file then invokes it with the correlation + payload, returning the outcome', async () => {
    scriptResults(undefined, { filled: true });

    const res = await send(request);

    expect(executeScriptMock).toHaveBeenNthCalledWith(1, { target: { tabId: 7 }, files: [file] });
    expect(executeScriptMock).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ target: { tabId: 7 }, args })
    );
    expect(res).toEqual({ ok: true, kind, result: { filled: true } });
  });

  it('surfaces the fail-safe not-found result straight through — never a different field', async () => {
    scriptResults(undefined, { filled: false, error: NOT_FOUND });

    expect(await send(request)).toEqual({
      ok: true,
      kind,
      result: { filled: false, error: NOT_FOUND },
    });
  });

  it('surfaces the failure message when the injected script returns a malformed result', async () => {
    scriptResults(undefined, null);

    expect(await send(request)).toEqual({ ok: false, error: malformed });
  });
});

describe('answerReplace request — changed since the pick', () => {
  it('surfaces the changed-since-pick refusal straight through — never overwrites a manual edit', async () => {
    paired();
    const refusal = {
      filled: false,
      error: 'This field changed since you picked it — re-pick it to rewrite.',
    };
    scriptResults(undefined, refusal);

    const res = await send({
      kind: 'answerReplace',
      question: 'Why this role?',
      index: 0,
      count: 1,
      text: 'A rewritten answer.',
      expectedValue: 'Because I like it.',
    });

    expect(res).toEqual({ ok: true, kind: 'answerReplace', result: refusal });
  });
});
