import { describe, expect, it } from 'vitest';

import {
  buildEmphasisBlock,
  buildEmphasisDirectivesBlock,
  buildGroundingBlock,
  buildLetterEmphasisBlock,
  buildSalaryRangeBlock,
  EMPHASIS_OPTIONS,
  type EmphasisId,
  resumeMentions,
  sourceCased,
} from '../index';
import { RESUME_FOR_GROUNDING } from '../test-support';

describe('buildEmphasisDirectivesBlock (#15)', () => {
  it('returns empty for no/empty selection', () => {
    expect(buildEmphasisDirectivesBlock(undefined)).toBe('');
    expect(buildEmphasisDirectivesBlock([])).toBe('');
  });

  it('emits one instruction per selected directive, in registry order, with a no-fabrication guard', () => {
    const block = buildEmphasisDirectivesBlock(['technical', 'quantify']);
    expect(block).toContain('WITHOUT inventing facts');
    // Registry order (quantify before technical) regardless of input order.
    expect(block.indexOf('Quantify impact')).toBeLessThan(block.indexOf('Technical depth'));
    // Exactly two directive lines.
    expect(block.split('\n').filter((l) => l.startsWith('- ')).length).toBe(2);
  });

  it('ignores unknown ids and de-dupes repeats', () => {
    // Cast simulates a stale/unknown id leaking from persisted state.
    const ids = ['quantify', 'quantify', 'bogus'] as EmphasisId[];
    const block = buildEmphasisDirectivesBlock(ids);
    expect(block.split('\n').filter((l) => l.startsWith('- ')).length).toBe(1);
  });

  it('every registry option carries a fact-safe instruction', () => {
    expect(EMPHASIS_OPTIONS.length).toBeGreaterThanOrEqual(5);
    for (const o of EMPHASIS_OPTIONS) {
      expect(o.instruction.length).toBeGreaterThan(20);
    }
  });
});

describe('resumeMentions', () => {
  it('matches single tokens on word boundaries (not substrings)', () => {
    expect(resumeMentions('Built React apps', 'React')).toBe(true);
    expect(resumeMentions('Worked in the category team', 'Go')).toBe(false);
    expect(resumeMentions('Wrote services in Go', 'Go')).toBe(true);
  });

  it('matches punctuated / multi-word terms as substrings', () => {
    expect(resumeMentions('Built with Node.js', 'node.js')).toBe(true);
    expect(resumeMentions('Designed a REST API for payments', 'REST API')).toBe(true);
    expect(resumeMentions('No cloud here', 'AWS')).toBe(false);
  });

  it('synonym path: JS alias matches JavaScript requirement', () => {
    // Résumé says "JS bundles"; requirement spells out "JavaScript".
    // The SYNONYMS map normalizes "js" → "javascript" on both sides.
    expect(resumeMentions('Shipped JS bundles and optimized load times', 'JavaScript')).toBe(true);
  });

  it('synonym path: k8s alias matches Kubernetes requirement', () => {
    // Résumé says "k8s clusters"; requirement spells out "Kubernetes".
    expect(resumeMentions('Ran k8s clusters on bare metal', 'Kubernetes')).toBe(true);
  });

  it('negative: java must NOT match javascript (word-boundary, no false alias)', () => {
    // "java" and "javascript" are different tokens; no synonym maps one to the other.
    expect(resumeMentions('Maintained Java microservices', 'javascript')).toBe(false);
  });

  it('punctuation edge: trailing comma on résumé token does not block alias match', () => {
    // "JavaScript," (trailing comma) must still match requirement "JavaScript".
    expect(
      resumeMentions('Shipped JavaScript, bundles and optimized load times', 'JavaScript')
    ).toBe(true);
  });

  it('punctuation edge: leading/trailing parens on résumé token do not block alias match', () => {
    // "(Kubernetes)" must still match requirement "Kubernetes".
    expect(resumeMentions('(Kubernetes) clusters on bare metal', 'Kubernetes')).toBe(true);
  });

  it('boundary trim: strips leading/trailing boundary punct, preserves internal punct', () => {
    // Trailing comma stripped → matches
    expect(resumeMentions('JavaScript, bundles shipped', 'JavaScript')).toBe(true);
    // Parens stripped → matches
    expect(resumeMentions('(Kubernetes) on-prem', 'Kubernetes')).toBe(true);
    // Internal punct preserved — c++ must not collapse to c
    expect(resumeMentions('shipped in c++', 'c++')).toBe(true);
    // Internal dot preserved — node.js must not collapse to node
    expect(resumeMentions('runs on node.js', 'node.js')).toBe(true);
  });

  it('redos regression: pathological punctuation token completes instantly (linear scan)', () => {
    // 100 000 consecutive quote chars — the old /^[...]+|[...]+$/g regex
    // backtracks polynomially on this input; the linear scan returns immediately.
    const pathological = '"'.repeat(100_000);
    const result = resumeMentions(pathological, 'JavaScript');
    // The entire token is boundary punctuation → stripped to '' → no match.
    expect(result).toBe(false);
  });
});

describe('buildGroundingBlock', () => {
  it('splits requirements into résumé-backed present vs absent', () => {
    const block = buildGroundingBlock(RESUME_FOR_GROUNDING, [
      'React',
      'TypeScript',
      'AWS',
      'Kubernetes',
    ]);
    expect(block).toContain('PRESENT');
    expect(block).toContain('React');
    expect(block).toContain('TypeScript');
    expect(block).toContain('ABSENT');
    expect(block).toContain('AWS');
    expect(block).toContain('Kubernetes');
  });

  it('returns empty string when there are no requirements', () => {
    expect(buildGroundingBlock(RESUME_FOR_GROUNDING, [])).toBe('');
  });
});

describe('buildSalaryRangeBlock (C2)', () => {
  it('renders only the validated integers and currency code as a fenced, labeled block', () => {
    const block = buildSalaryRangeBlock({ min: 65000, max: 80000, currency: 'EUR' });
    expect(block).toContain('<salary_context>');
    expect(block).toContain('65000');
    expect(block).toContain('80000');
    expect(block).toContain('EUR');
  });

  it('is source-neutral — never claims the range is web-sourced (it may be employer-stated scraped data)', () => {
    const block = buildSalaryRangeBlock({ min: 65000, max: 80000, currency: 'EUR' });
    expect(block).not.toMatch(/web/i);
  });

  it('is empty for no range, or a structurally invalid one (defense in depth)', () => {
    expect(buildSalaryRangeBlock(undefined)).toBe('');
    expect(buildSalaryRangeBlock({ min: 0, max: 80000, currency: 'EUR' })).toBe('');
    expect(buildSalaryRangeBlock({ min: 90000, max: 80000, currency: 'EUR' })).toBe('');
  });

  it('is empty for a structurally invalid currency code (self-defending, not just trusting Rust)', () => {
    for (const currency of ['', 'U', 'US', 'TOOLONG', '12A', 'eu-r']) {
      expect(buildSalaryRangeBlock({ min: 65000, max: 80000, currency })).toBe('');
    }
  });

  it('accepts a 4-letter currency code', () => {
    expect(buildSalaryRangeBlock({ min: 1, max: 2, currency: 'USDX' })).toContain('USDX');
  });
});

describe('emphasis keeps the résumé spelling', () => {
  const resume = 'Built the client in React and Node.js, deployed on AWS with CI/CD.';

  it('maps a normalized lowercase key back to the source casing', () => {
    expect(sourceCased('react', resume)).toBe('React');
    expect(sourceCased('node.js', resume)).toBe('Node.js');
    expect(sourceCased('aws', resume)).toBe('AWS');
    expect(sourceCased('ci/cd', resume)).toBe('CI/CD');
  });

  it('matches whole words only and leaves an unmentioned term untouched', () => {
    expect(sourceCased('go', 'Category management')).toBe('go');
    expect(sourceCased('kubernetes', resume)).toBe('kubernetes');
    expect(sourceCased('react')).toBe('react');
  });

  it('prefers the mixed-case spelling over a heading or a prose word', () => {
    const body = 'SKILLS: REACT\nBuilt the go-to-market plan. Languages: Go, React.';
    expect(sourceCased('react', body)).toBe('React');
    expect(sourceCased('go', body)).toBe('Go');
    expect(sourceCased('react', 'REACT only')).toBe('REACT');
    expect(sourceCased('aws', 'Aws notes, AWS usage')).toBe('AWS');
    expect(sourceCased('go', 'GO TO MARKET plan; Go services')).toBe('Go');
    expect(sourceCased('rust', 'RUST developer; Rust services')).toBe('Rust');
    expect(sourceCased('sap', 'Sap notes, SAP usage')).toBe('SAP');
    expect(sourceCased('typescript', 'Typescript and TypeScript')).toBe('TypeScript');
    expect(sourceCased('rest', 'Rest of the team built REST APIs')).toBe('REST');
    expect(sourceCased('it', 'It supports IT infrastructure')).toBe('IT');
  });

  it('bolds the source spelling in both emphasis blocks, never the key', () => {
    const resumeBlock = buildEmphasisBlock(['react', 'aws'], resume);
    expect(resumeBlock).toContain('**React**');
    expect(resumeBlock).toContain('**AWS**');
    expect(resumeBlock).not.toContain('**react**');
    const letterBlock = buildLetterEmphasisBlock(['react', 'ai'], `${resume} Some AI work.`);
    expect(letterBlock).toContain('**React**');
    expect(letterBlock).toContain('**AI**');
    expect(letterBlock).not.toContain('**ai**');
  });
});
