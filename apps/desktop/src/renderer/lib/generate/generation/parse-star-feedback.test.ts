import { describe, expect, it } from 'vitest';

import { parseStarFeedback } from './parse-star-feedback';

describe('parseStarFeedback', () => {
  it('parses a clean, fully-sectioned response', () => {
    const raw = [
      'STRENGTHS:',
      '- Clear ownership of the migration',
      '- Named a concrete metric',
      '',
      'GAPS:',
      '- Did not mention stakeholder communication',
      '',
      'STAR:',
      'SITUATION: present',
      'TASK: present',
      'ACTION: missing',
      'RESULT: present',
      '',
      'REWRITE:',
      'I led the database migration end to end, cutting downtime from 4 hours to 20 minutes.',
    ].join('\n');

    const out = parseStarFeedback(raw);

    expect(out.strengths).toEqual(['Clear ownership of the migration', 'Named a concrete metric']);
    expect(out.gaps).toEqual(['Did not mention stakeholder communication']);
    expect(out.star).toEqual({ situation: true, task: true, action: false, result: true });
    expect(out.rewrite).toBe(
      'I led the database migration end to end, cutting downtime from 4 hours to 20 minutes.'
    );
  });

  it('tolerates inline content directly after the section marker', () => {
    const raw =
      'STRENGTHS: Good structure\nGAPS: Missed a concrete result\nSTAR:\nSITUATION: missing';
    const out = parseStarFeedback(raw);
    expect(out.strengths).toEqual(['Good structure']);
    expect(out.gaps).toEqual(['Missed a concrete result']);
    expect(out.star.situation).toBe(false);
  });

  it('defaults every STAR field to missing (false) when the section is absent', () => {
    const out = parseStarFeedback('STRENGTHS:\n- Good energy\n');
    expect(out.star).toEqual({ situation: false, task: false, action: false, result: false });
  });

  it('is lenient about STAR value casing/variants ("Yes"/"true" == present)', () => {
    const raw = 'STAR:\nSITUATION: Yes\nTASK: true\nACTION: No\nRESULT: Missing';
    const out = parseStarFeedback(raw);
    expect(out.star).toEqual({ situation: true, task: true, action: false, result: false });
  });

  it('returns empty strengths/gaps and an empty rewrite for empty input', () => {
    const out = parseStarFeedback('');
    expect(out.strengths).toEqual([]);
    expect(out.gaps).toEqual([]);
    expect(out.rewrite).toBe('');
    expect(out.star).toEqual({ situation: false, task: false, action: false, result: false });
  });

  it('joins a multi-line rewrite into one string', () => {
    const raw = 'REWRITE:\nFirst sentence of the rewrite.\nSecond sentence continues it.';
    const out = parseStarFeedback(raw);
    expect(out.rewrite).toBe('First sentence of the rewrite. Second sentence continues it.');
  });

  it('drops the literal "None" gaps marker instead of rendering it as a false gap', () => {
    const bulleted = parseStarFeedback('GAPS:\n- None\n');
    expect(bulleted.gaps).toEqual([]);

    // Case-insensitive, and also when it lands inline on the section header.
    const inline = parseStarFeedback('GAPS: none');
    expect(inline.gaps).toEqual([]);

    const upper = parseStarFeedback('GAPS:\nNONE');
    expect(upper.gaps).toEqual([]);

    // A real gap that merely starts with "None" some other way is still kept.
    const realGap = parseStarFeedback('GAPS:\n- None of the metrics were quantified.');
    expect(realGap.gaps).toEqual(['None of the metrics were quantified.']);
  });
});
