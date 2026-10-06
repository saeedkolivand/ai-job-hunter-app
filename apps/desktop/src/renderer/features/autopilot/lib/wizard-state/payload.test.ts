import { describe, expect, it } from 'vitest';

import type { WizardState } from '@/features/autopilot/types';

import { wizardStateToPayload } from '../wizard-state';

// ── wizardStateToPayload ──────────────────────────────────────────────────────

function makeForm(overrides: Partial<WizardState> = {}): WizardState {
  return {
    name: 'Backend roles',
    boards: ['linkedin'],
    query: 'rust backend',
    location: 'Berlin',
    workTypes: ['remote'],
    pages: 3,
    dateFilter: '24h',
    watchedCompaniesOnly: false,
    minMatchScore: 70,
    keywords: 'rust, tokio',
    excludeKeywords: 'php',
    resumeText: 'my resume',
    assistant: false,
    schedule: 'daily',
    scheduleHour: 9,
    scheduleMinute: 30,
    ...overrides,
  };
}

describe('wizardStateToPayload()', () => {
  it('maps a fully-populated form onto the create payload', () => {
    expect(wizardStateToPayload(makeForm())).toEqual({
      name: 'Backend roles',
      target: {
        boards: ['linkedin'],
        query: 'rust backend',
        location: 'Berlin',
        workTypes: ['remote'],
        pages: 3,
        dateFilter: '24h',
      },
      filter: {
        minMatchScore: 70,
        keywords: ['rust', 'tokio'],
        excludeKeywords: ['php'],
      },
      resumeText: 'my resume',
      assistant: false,
      schedule: 'daily',
      scheduleHour: 9,
      scheduleMinute: 30,
    });
  });

  describe('keyword splitting', () => {
    it('trims, splits on comma, and drops empty fragments', () => {
      expect(
        wizardStateToPayload(makeForm({ keywords: ' rust ,, tokio , ' })).filter.keywords
      ).toEqual(['rust', 'tokio']);
    });

    it('maps an empty / whitespace-only / comma-only string to undefined', () => {
      expect(wizardStateToPayload(makeForm({ keywords: '' })).filter.keywords).toBeUndefined();
      expect(wizardStateToPayload(makeForm({ keywords: '   ' })).filter.keywords).toBeUndefined();
      expect(wizardStateToPayload(makeForm({ keywords: ', ,' })).filter.keywords).toBeUndefined();
    });

    it('applies the same rule to excludeKeywords', () => {
      expect(
        wizardStateToPayload(makeForm({ excludeKeywords: '' })).filter.excludeKeywords
      ).toBeUndefined();
      expect(
        wizardStateToPayload(makeForm({ excludeKeywords: 'java, c#' })).filter.excludeKeywords
      ).toEqual(['java', 'c#']);
    });
  });

  describe('pages passthrough (true pages field)', () => {
    // The form used to hold an item count divided by 25 on save, so anything
    // above 250 silently collapsed to the same 10 pages. It now carries the
    // page budget verbatim across the whole range the widget/schema allow.
    it.each([1, 2, 3, 7, 10])('forwards pages: %i to target.pages unchanged', (pages) => {
      expect(wizardStateToPayload(makeForm({ pages })).target.pages).toBe(pages);
    });
  });

  describe('workTypes', () => {
    it("drops workTypes when the array is empty (the 'any' sentinel)", () => {
      expect(wizardStateToPayload(makeForm({ workTypes: [] })).target.workTypes).toBeUndefined();
    });

    it('keeps a one-item workTypes array verbatim', () => {
      expect(wizardStateToPayload(makeForm({ workTypes: ['hybrid'] })).target.workTypes).toEqual([
        'hybrid',
      ]);
    });

    it('keeps a multi-item workTypes array verbatim', () => {
      expect(
        wizardStateToPayload(makeForm({ workTypes: ['remote', 'on-site'] })).target.workTypes
      ).toEqual(['remote', 'on-site']);
    });
  });

  describe('schedule time', () => {
    it('drops hour/minute for a manual schedule', () => {
      const payload = wizardStateToPayload(makeForm({ schedule: 'manual' }));
      expect(payload.scheduleHour).toBeUndefined();
      expect(payload.scheduleMinute).toBeUndefined();
    });

    it('keeps hour/minute for recurring schedules', () => {
      const payload = wizardStateToPayload(
        makeForm({ schedule: 'twice_daily', scheduleHour: 6, scheduleMinute: 15 })
      );
      expect(payload.scheduleHour).toBe(6);
      expect(payload.scheduleMinute).toBe(15);
    });
  });

  describe('empty optionals collapse to undefined', () => {
    it('drops empty location, dateFilter, and resumeText', () => {
      const payload = wizardStateToPayload(
        makeForm({ location: '', dateFilter: '', resumeText: '' })
      );
      expect(payload.target.location).toBeUndefined();
      expect(payload.target.dateFilter).toBeUndefined();
      expect(payload.resumeText).toBeUndefined();
    });

    it('maps dateFilter: "" → undefined in target (Fix C regression lock)', () => {
      // The new buildDefaults() seeds '' (not '24h'). This confirms '' collapses
      // to undefined on the wire so no date restriction is forwarded to the scraper.
      const payload = wizardStateToPayload(makeForm({ dateFilter: '' }));
      expect(payload.target.dateFilter).toBeUndefined();
    });

    it('passes minMatchScore: 0 through to filter.minMatchScore (Fix C regression lock)', () => {
      // 0 is a legitimate "keep everything" threshold; it must not be treated as
      // falsy and dropped. The payload must carry 0 as-is.
      const payload = wizardStateToPayload(makeForm({ minMatchScore: 0 }));
      expect(payload.filter.minMatchScore).toBe(0);
    });
  });

  describe('watchedCompaniesOnly forwarding (ADR-030 §e)', () => {
    it('forwards watchedCompaniesOnly: true to the target', () => {
      const payload = wizardStateToPayload(makeForm({ watchedCompaniesOnly: true }));
      expect(payload.target.watchedCompaniesOnly).toBe(true);
    });

    it('collapses watchedCompaniesOnly: false to undefined (old autopilots stay clean)', () => {
      const payload = wizardStateToPayload(makeForm({ watchedCompaniesOnly: false }));
      expect(payload.target.watchedCompaniesOnly).toBeUndefined();
    });
  });

  describe('countryCode forwarding (Fix A)', () => {
    it('forwards a non-empty countryCode from the form to target', () => {
      const payload = wizardStateToPayload(makeForm({ countryCode: 'gb' }));
      expect(payload.target.countryCode).toBe('gb');
    });

    it('drops countryCode when it is undefined in the form', () => {
      const payload = wizardStateToPayload(makeForm({ countryCode: undefined }));
      expect(payload.target.countryCode).toBeUndefined();
    });

    it('drops countryCode when it is an empty string in the form', () => {
      // The || undefined guard collapses '' to undefined so it is not forwarded.
      const payload = wizardStateToPayload(makeForm({ countryCode: '' }));
      expect(payload.target.countryCode).toBeUndefined();
    });
  });

  describe('assistant (Phase 4 AI notes) snapshot forwarding', () => {
    it('forwards the provider snapshot when assistant is enabled', () => {
      const payload = wizardStateToPayload(
        makeForm({
          assistant: true,
          assistantProvider: 'openai',
          assistantModel: 'gpt-4o',
          assistantBaseUrl: 'https://api.example.com',
        })
      );
      expect(payload.assistant).toBe(true);
      expect(payload.assistantProvider).toBe('openai');
      expect(payload.assistantModel).toBe('gpt-4o');
      expect(payload.assistantBaseUrl).toBe('https://api.example.com');
    });

    it('clears the provider snapshot when assistant is disabled, even if the form still carries stale values', () => {
      const payload = wizardStateToPayload(
        makeForm({
          assistant: false,
          assistantProvider: 'openai',
          assistantModel: 'gpt-4o',
          assistantBaseUrl: 'https://api.example.com',
        })
      );
      expect(payload.assistant).toBe(false);
      expect(payload.assistantProvider).toBeUndefined();
      expect(payload.assistantModel).toBeUndefined();
      expect(payload.assistantBaseUrl).toBeUndefined();
    });
  });
});
