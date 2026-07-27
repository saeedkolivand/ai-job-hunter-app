import { describe, expect, it } from 'vitest';

import { BOARD_IDS } from '@ajh/shared';

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
    pages: 2,
    dateFilter: '',
    watchedCompaniesOnly: false,
    minMatchScore: 50,
    keywords: '',
    excludeKeywords: '',
    resumeText: '',
    assistant: false,
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

  it('accepts a full-catalog-sized board selection (no upper bound)', () => {
    expect(BOARD_IDS.length).toBeGreaterThan(0);
    expect(autopilotWizardSchema.safeParse(makeForm({ boards: [...BOARD_IDS] })).success).toBe(
      true
    );
  });

  it('rejects a boards array containing an empty string (per-item min(1))', () => {
    expect(autopilotWizardSchema.safeParse(makeForm({ boards: [''] })).success).toBe(false);
  });

  it('rejects out-of-range numeric controls (pages outside 1–10, score > 100)', () => {
    // pages mirrors the backend AutopilotTargetSchema range, so 11 (and 0) must
    // fail here rather than reaching the IPC boundary.
    expect(autopilotWizardSchema.safeParse(makeForm({ pages: 11 })).success).toBe(false);
    expect(autopilotWizardSchema.safeParse(makeForm({ pages: 0 })).success).toBe(false);
    expect(autopilotWizardSchema.safeParse(makeForm({ minMatchScore: 101 })).success).toBe(false);
  });

  it('rejects a non-integer page count', () => {
    // NumberField clamps its range on blur but does not round, so this is the
    // one way a user can hand the wizard an invalid `pages`. StepTarget rounds
    // on blur and CreationWizard gates `pages` on step 0 because of it.
    expect(autopilotWizardSchema.safeParse(makeForm({ pages: 2.5 })).success).toBe(false);
  });

  it('reports every pages failure as the same i18n KEY, not a zod English default', () => {
    // StepTarget renders this straight through t(...) into WizardField's error
    // slot; a raw zod message would ship untranslated copy to the user.
    const key = 'autopilot.wizard.validation.pagesRange';
    expect(messageFor(makeForm({ pages: 2.5 }), 'pages')).toBe(key);
    expect(messageFor(makeForm({ pages: 0 }), 'pages')).toBe(key);
    expect(messageFor(makeForm({ pages: 11 }), 'pages')).toBe(key);
  });

  it('accepts the full 1–10 page range', () => {
    expect(autopilotWizardSchema.safeParse(makeForm({ pages: 1 })).success).toBe(true);
    expect(autopilotWizardSchema.safeParse(makeForm({ pages: 10 })).success).toBe(true);
  });
});
