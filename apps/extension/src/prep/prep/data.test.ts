import { describe, expect, it } from 'vitest';

import { parsePrepResourceData, prepHasContent } from '../prep';

const EMPTY_DATA = {
  hasCompanyBrief: false,
  companyBrief: null,
  interviewQuestions: [],
  salaryAnswer: null,
};

describe('parsePrepResourceData', () => {
  it('parses a full payload', () => {
    const data = parsePrepResourceData({
      generation: {
        hasCompanyBrief: true,
        companyBrief: 'Acme makes widgets.',
        interviewQuestions: [
          { question: 'Tell me about yourself', why: 'Warm-up', audience: 'recruiter' },
          { question: 'Why us?' },
        ],
        salaryAnswer: 'I am targeting $120k-$140k.',
        updatedAt: 123,
      },
    });
    expect(data).toEqual({
      hasCompanyBrief: true,
      companyBrief: 'Acme makes widgets.',
      interviewQuestions: [
        { question: 'Tell me about yourself', why: 'Warm-up', audience: 'recruiter' },
        { question: 'Why us?', why: undefined, audience: undefined },
      ],
      salaryAnswer: 'I am targeting $120k-$140k.',
    });
  });

  it('degrades to empty on malformed/missing data (never throws)', () => {
    for (const malformed of [null, undefined, 'nope', {}, { generation: null }]) {
      expect(parsePrepResourceData(malformed)).toEqual(EMPTY_DATA);
    }
  });

  it('drops a malformed interview-question entry (missing question) without failing the whole list', () => {
    const data = parsePrepResourceData({
      generation: {
        hasCompanyBrief: false,
        interviewQuestions: [{ why: 'no question text' }, { question: 'Good one?' }],
      },
    });
    expect(data.interviewQuestions).toEqual([
      { question: 'Good one?', why: undefined, audience: undefined },
    ]);
  });
});

describe('prepHasContent', () => {
  it('false when the job has nothing yet', () => {
    expect(prepHasContent(EMPTY_DATA)).toBe(false);
  });

  it('true when only interview questions exist', () => {
    expect(prepHasContent({ ...EMPTY_DATA, interviewQuestions: [{ question: 'Why us?' }] })).toBe(
      true
    );
  });
});
