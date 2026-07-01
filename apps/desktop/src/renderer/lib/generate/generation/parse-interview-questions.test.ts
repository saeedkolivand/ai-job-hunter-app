import { describe, expect, it } from 'vitest';

import { parseInterviewQuestions } from './parse-interview-questions';

describe('parseInterviewQuestions', () => {
  it('parses a clean delimited list into question / why / audience', () => {
    const raw = [
      'Q: How does the team measure success for this role?',
      'WHY: Shows outcome focus.',
      'AUDIENCE: hiringManager',
      '',
      'Q: What is the biggest technical challenge right now?',
      'WHY: Signals engagement with real problems.',
      'AUDIENCE: team',
    ].join('\n');

    const out = parseInterviewQuestions(raw);

    expect(out).toHaveLength(2);
    expect(out[0]).toMatchObject({
      id: 'iq-1',
      question: 'How does the team measure success for this role?',
      why: 'Shows outcome focus.',
      audience: 'hiringManager',
    });
    expect(out[1]?.audience).toBe('team');
  });

  it('tolerates numbering / bullet prefixes on the Q line', () => {
    const raw = '1. Q: First?\nWHY: w\nAUDIENCE: recruiter\n\n- Q: Second?\nAUDIENCE: team';
    const out = parseInterviewQuestions(raw);
    expect(out.map((q) => q.question)).toEqual(['First?', 'Second?']);
  });

  it('defaults a missing why to empty and a missing/unknown audience to general', () => {
    const raw = 'Q: Only a question?\n\nQ: Another?\nAUDIENCE: nonsense';
    const out = parseInterviewQuestions(raw);
    expect(out).toHaveLength(2);
    expect(out[0]).toMatchObject({ why: '', audience: 'general' });
    expect(out[1]?.audience).toBe('general');
  });

  it('normalizes audience variants (case / spacing) to known ids', () => {
    const raw = 'Q: a?\nAUDIENCE: Hiring Manager\n\nQ: b?\nAUDIENCE: LEADERSHIP';
    const out = parseInterviewQuestions(raw);
    expect(out[0]?.audience).toBe('hiringManager');
    expect(out[1]?.audience).toBe('leadership');
  });

  it('skips blocks with no question text and returns [] for empty input', () => {
    expect(parseInterviewQuestions('')).toEqual([]);
    expect(parseInterviewQuestions('WHY: orphan\nAUDIENCE: team')).toEqual([]);
  });
});
