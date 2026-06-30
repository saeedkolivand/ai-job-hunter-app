import { describe, expect, it } from 'vitest';

import { createThinkSplitter } from './think-split';

/** Drive a splitter with a list of deltas, then flush, collecting both channels. */
function run(deltas: string[]): { answer: string; thinking: string } {
  let answer = '';
  let thinking = '';
  const splitter = createThinkSplitter(
    (t) => {
      answer += t;
    },
    (t) => {
      thinking += t;
    }
  );
  for (const d of deltas) splitter.push(d);
  splitter.flush();
  return { answer, thinking };
}

describe('createThinkSplitter', () => {
  it('passes plain content straight through to onToken', () => {
    expect(run(['Hello ', 'world'])).toEqual({ answer: 'Hello world', thinking: '' });
  });

  it('routes an inline <think> block to onThinking and the rest to onToken', () => {
    const { answer, thinking } = run(['<think>reasoning here</think>the answer']);
    expect(thinking).toBe('reasoning here');
    expect(answer).toBe('the answer');
  });

  it('detects a <think> tag split across deltas', () => {
    // The opening tag arrives in two chunks: "<thi" then "nk>".
    const { answer, thinking } = run(['before <thi', 'nk>secret</think> after']);
    expect(answer).toBe('before  after');
    expect(thinking).toBe('secret');
  });

  it('streams reasoning across multiple deltas while inside a block', () => {
    const { answer, thinking } = run(['<think>part one ', 'part two</think>done']);
    expect(thinking).toBe('part one part two');
    expect(answer).toBe('done');
  });

  it('discards an unclosed think block on flush (no leak into the answer)', () => {
    const { answer, thinking } = run(['visible <think>never closes...']);
    expect(answer).toBe('visible ');
    expect(thinking).toBe('never closes...');
  });

  it('handles several think blocks in one stream', () => {
    const { answer, thinking } = run(['a<think>x</think>b<think>y</think>c']);
    expect(answer).toBe('abc');
    expect(thinking).toBe('xy');
  });

  it('flush emits trailing held-back content shorter than a tag', () => {
    // "end" is shorter than "<think>" so it is held back until flush.
    const { answer } = run(['the end']);
    expect(answer).toBe('the end');
  });
});
