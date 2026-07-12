import { describe, expect, it } from 'vitest';

import { parseLikelyQuestions } from './parse-likely-questions';

describe('parseLikelyQuestions', () => {
  it('parses a clean delimited list into question / type', () => {
    const raw = [
      'Q: Tell me about a time you led a project under a tight deadline.',
      'TYPE: behavioral',
      '',
      'Q: How would you design our rate-limiting layer?',
      'TYPE: technical',
    ].join('\n');

    const out = parseLikelyQuestions(raw);

    expect(out).toHaveLength(2);
    expect(out[0]).toMatchObject({
      id: 'lq-1',
      question: 'Tell me about a time you led a project under a tight deadline.',
      type: 'behavioral',
    });
    expect(out[1]?.type).toBe('technical');
  });

  it('tolerates numbering / bullet prefixes on the Q line', () => {
    const raw = '1. Q: First?\nTYPE: technical\n\n- Q: Second?\nTYPE: behavioral';
    const out = parseLikelyQuestions(raw);
    expect(out.map((q) => q.question)).toEqual(['First?', 'Second?']);
  });

  it('defaults a missing/unknown type to roleSpecific', () => {
    const raw = 'Q: Only a question?\n\nQ: Another?\nTYPE: nonsense';
    const out = parseLikelyQuestions(raw);
    expect(out).toHaveLength(2);
    expect(out[0]?.type).toBe('roleSpecific');
    expect(out[1]?.type).toBe('roleSpecific');
  });

  it('normalizes type variants (case / spacing) to known ids', () => {
    const raw = 'Q: a?\nTYPE: Role Specific\n\nQ: b?\nTYPE: TECHNICAL';
    const out = parseLikelyQuestions(raw);
    expect(out[0]?.type).toBe('roleSpecific');
    expect(out[1]?.type).toBe('technical');
  });

  it('skips blocks with no question text and returns [] for empty input', () => {
    expect(parseLikelyQuestions('')).toEqual([]);
    expect(parseLikelyQuestions('TYPE: technical')).toEqual([]);
  });
});
