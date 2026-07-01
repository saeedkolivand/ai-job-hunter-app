import { describe, expect, it } from 'vitest';

import type { WizardState } from '@/features/autopilot/types';

import { autopilotWizardSchema } from './schema';

/** A fully valid wizard form — the step-0 gate (name/boards/query) is satisfied. */
function makeForm(overrides: Partial<WizardState> = {}): WizardState {
  return {
    name: 'Backend roles',
    boards: ['linkedin'],
    query: 'rust backend',
    location: '',
    workType: 'any',
    amount: 50,
    dateFilter: '',
    minMatchScore: 50,
    keywords: '',
    excludeKeywords: '',
    resumeText: '',
    schedule: 'daily',
    scheduleHour: 9,
    scheduleMinute: 0,
    ...overrides,
  };
}

/** First issue message (i18n key) for a given dotted field path, if any. */
function messageFor(form: WizardState, field: keyof WizardState): string | undefined {
  const result = autopilotWizardSchema.safeParse(form);
  if (result.success) return undefined;
  return result.error.issues.find((i) => i.path[0] === field)?.message;
}

describe('autopilotWizardSchema — step-0 gate', () => {
  it('accepts a fully valid form', () => {
    expect(autopilotWizardSchema.safeParse(makeForm()).success).toBe(true);
  });

  it('requires name — empty fails with the nameRequired key', () => {
    expect(messageFor(makeForm({ name: '' }), 'name')).toBe(
      'autopilot.wizard.validation.nameRequired'
    );
  });

  it('treats a whitespace-only name as empty (trimmed)', () => {
    expect(messageFor(makeForm({ name: '   ' }), 'name')).toBe(
      'autopilot.wizard.validation.nameRequired'
    );
  });

  it('requires query — empty fails with the queryRequired key', () => {
    expect(messageFor(makeForm({ query: '' }), 'query')).toBe(
      'autopilot.wizard.validation.queryRequired'
    );
  });

  it('treats a whitespace-only query as empty (trimmed)', () => {
    expect(messageFor(makeForm({ query: '  ' }), 'query')).toBe(
      'autopilot.wizard.validation.queryRequired'
    );
  });

  it('requires at least one board in boards', () => {
    expect(autopilotWizardSchema.safeParse(makeForm({ boards: [] })).success).toBe(false);
  });

  it('rejects more than 6 boards', () => {
    expect(
      autopilotWizardSchema.safeParse(makeForm({ boards: ['a', 'b', 'c', 'd', 'e', 'f', 'g'] }))
        .success
    ).toBe(false);
  });

  it('rejects a boards array containing an empty string (per-item min(1))', () => {
    expect(autopilotWizardSchema.safeParse(makeForm({ boards: [''] })).success).toBe(false);
  });

  it('rejects out-of-range numeric controls (amount > 500, score > 100)', () => {
    expect(autopilotWizardSchema.safeParse(makeForm({ amount: 501 })).success).toBe(false);
    expect(autopilotWizardSchema.safeParse(makeForm({ minMatchScore: 101 })).success).toBe(false);
  });
});
