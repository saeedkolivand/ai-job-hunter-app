import { describe, expect, it } from 'vitest';

import { tailorWizardSchema } from './tailor-schema';
import type { TailorWizardState } from './tailor-state';

// ── Helpers ───────────────────────────────────────────────────────────────────

type ValidInput = TailorWizardState;

function makeValid(overrides: Partial<ValidInput> = {}): ValidInput {
  return {
    resume: 'My professional resume text',
    outputType: 'both',
    researchCompany: false,
    ...overrides,
  };
}

/** First issue message for a given field path, or undefined when the parse succeeds. */
function messageFor(input: ValidInput, field: string): string | undefined {
  const result = tailorWizardSchema.safeParse(input);
  if (result.success) return undefined;
  return result.error.issues.find((i) => i.path[0] === field)?.message;
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('tailorWizardSchema — valid input', () => {
  it('accepts a fully valid form', () => {
    expect(tailorWizardSchema.safeParse(makeValid()).success).toBe(true);
  });

  it('accepts all three valid outputType values', () => {
    expect(tailorWizardSchema.safeParse(makeValid({ outputType: 'resume' })).success).toBe(true);
    expect(tailorWizardSchema.safeParse(makeValid({ outputType: 'cover' })).success).toBe(true);
    expect(tailorWizardSchema.safeParse(makeValid({ outputType: 'both' })).success).toBe(true);
  });

  it('accepts researchCompany = true', () => {
    expect(tailorWizardSchema.safeParse(makeValid({ researchCompany: true })).success).toBe(true);
  });
});

describe('tailorWizardSchema — resume validation', () => {
  it('fails with the resumeRequired key when resume is empty', () => {
    expect(messageFor(makeValid({ resume: '' }), 'resume')).toBe(
      'autopilot.apply.wizard.validation.resumeRequired'
    );
  });

  it('fails with the resumeRequired key when resume is whitespace-only', () => {
    expect(messageFor(makeValid({ resume: '   ' }), 'resume')).toBe(
      'autopilot.apply.wizard.validation.resumeRequired'
    );
  });

  it('fails with the resumeRequired key when resume is a tab/newline string', () => {
    expect(messageFor(makeValid({ resume: '\t\n' }), 'resume')).toBe(
      'autopilot.apply.wizard.validation.resumeRequired'
    );
  });
});

describe('tailorWizardSchema — outputType validation', () => {
  it('rejects an invalid outputType value', () => {
    const result = tailorWizardSchema.safeParse(makeValid({ outputType: 'all' as 'both' }));
    expect(result.success).toBe(false);
  });

  it('rejects an empty outputType', () => {
    const result = tailorWizardSchema.safeParse(makeValid({ outputType: '' as 'both' }));
    expect(result.success).toBe(false);
  });
});

describe('tailorWizardSchema — researchCompany validation', () => {
  it('rejects a non-boolean researchCompany', () => {
    const result = tailorWizardSchema.safeParse({ ...makeValid(), researchCompany: 'yes' });
    expect(result.success).toBe(false);
  });

  it('accepts false as researchCompany', () => {
    expect(tailorWizardSchema.safeParse(makeValid({ researchCompany: false })).success).toBe(true);
  });

  it('accepts true as researchCompany', () => {
    expect(tailorWizardSchema.safeParse(makeValid({ researchCompany: true })).success).toBe(true);
  });
});
