import { describe, expect, it } from 'vitest';

import type { ContentReportPayload } from '@ajh/shared/ipc';

import { buildSectionVerdicts, sectionKeyForHeading } from './section-verdicts';

const METRICS: ContentReportPayload['metrics'] = {
  keywordCoverage: 50,
  topRequirementHits: 1,
  topRequirementsMeasured: 2,
  duplicateRatio: 0,
  rolesSource: 2,
  rolesOutput: 2,
};

function report(issues: ContentReportPayload['issues']): ContentReportPayload {
  return { ok: issues.length === 0, issues, metrics: METRICS };
}

const DOCUMENT = ['Summary', 'A backend engineer.', '', 'Experience', 'Acme — Engineer'].join('\n');

describe('sectionKeyForHeading', () => {
  it.each([
    ['Summary', 'summary'],
    ['Professional Summary', 'summary'],
    ['Kurzprofil', 'summary'],
    ['Technical Skills', 'skills'],
    ['Kenntnisse', 'skills'],
    ['Work Experience', 'experience:0'],
    ['BERUFSERFAHRUNG', 'experience:0'],
    ['Projects', 'projects'],
    ['Ausbildung', 'education'],
  ])('maps "%s" to %s', (heading, key) => {
    expect(sectionKeyForHeading(heading)).toBe(key);
  });

  it('returns null for a heading it cannot name — never a guessed key', () => {
    // A guessed key would be rejected at the IPC boundary anyway, and guessing
    // `header` in particular is what ADR-0021 forbids.
    expect(sectionKeyForHeading('Volunteering')).toBeNull();
    expect(sectionKeyForHeading('')).toBeNull();
    expect(sectionKeyForHeading('Header')).toBeNull();
  });
});

describe('buildSectionVerdicts', () => {
  it('returns nothing without a report', () => {
    expect(buildSectionVerdicts(null, DOCUMENT)).toEqual([]);
  });

  it('counts issues per section heading and marks the criticals', () => {
    const verdicts = buildSectionVerdicts(
      report([
        {
          severity: 'critical',
          code: 'factual.dropped_role',
          section: 'Experience',
          message: 'x',
          evidence: null,
        },
        {
          severity: 'warning',
          code: 'ats.long_bullet',
          section: 'Experience',
          message: 'x',
          evidence: null,
        },
      ]),
      DOCUMENT
    );
    const experience = verdicts.find((v) => v.label === 'Experience');
    expect(experience).toMatchObject({ sectionKey: 'experience:0', issues: 2, criticals: 1 });
  });

  it('puts flagged sections before clean ones', () => {
    const verdicts = buildSectionVerdicts(
      report([
        {
          severity: 'warning',
          code: 'ats.long_bullet',
          section: 'Experience',
          message: 'x',
          evidence: null,
        },
      ]),
      DOCUMENT
    );
    expect(verdicts[0]?.label).toBe('Experience');
    expect(verdicts.map((v) => v.sectionKey)).toContain('summary');
  });

  it('skips document-wide findings — they belong to no section', () => {
    const verdicts = buildSectionVerdicts(
      report([
        {
          severity: 'critical',
          code: 'content.language_mismatch',
          section: null,
          message: 'x',
          evidence: null,
        },
      ]),
      DOCUMENT
    );
    expect(verdicts.every((v) => v.issues === 0)).toBe(true);
  });

  // A "no changes needed" chip is a CLAIM that the section was checked. It is
  // only earned when the heading is actually in the document — otherwise the
  // panel would report a clean Education section for a résumé with none.
  it('only calls a section clean when its heading is present in the document', () => {
    const verdicts = buildSectionVerdicts(report([]), DOCUMENT);
    const keys = verdicts.map((v) => v.sectionKey);
    expect(keys).toEqual(expect.arrayContaining(['summary', 'experience:0']));
    expect(keys).not.toContain('education');
    expect(keys).not.toContain('projects');
  });

  it('does not mistake a punctuated body line for a heading', () => {
    const prose = 'Experience\nLed the projects rewrite.';
    const keys = buildSectionVerdicts(report([]), prose).map((v) => v.sectionKey);
    expect(keys).toEqual(['experience:0']);
  });

  it('does not mistake a long unpunctuated content line for a heading', () => {
    // A packed skills/keyword line carries no sentence punctuation, so LENGTH
    // is the only thing separating it from a real heading.
    const packed = [
      'Experience',
      'Skills Python JavaScript Rust Golang Kubernetes Terraform PostgreSQL Redis Kafka',
    ].join('\n');
    const keys = buildSectionVerdicts(report([]), packed).map((v) => v.sectionKey);
    expect(keys).toEqual(['experience:0']);
  });

  it('shows an unmappable heading with no section key, so no Fix button is offered', () => {
    const verdicts = buildSectionVerdicts(
      report([
        {
          severity: 'warning',
          code: 'ats.long_bullet',
          section: 'Volunteering',
          message: 'x',
          evidence: null,
        },
      ]),
      DOCUMENT
    );
    expect(verdicts.find((v) => v.label === 'Volunteering')?.sectionKey).toBeNull();
  });
});
