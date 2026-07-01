/**
 * Settings search — unit tests.
 *
 * Covers:
 *  1. matchEntries returns [] for empty / whitespace query.
 *  2. matchEntries matches on localized title (case-insensitive) — specific result asserted.
 *  3. matchEntries matches on a keyword — specific result asserted.
 *  4. matchEntries matches on the localized section label — specific result asserted.
 *  5. matchEntries returns [] when nothing matches (no-results path).
 *  6. matchEntries trims surrounding whitespace from the query.
 *  7. SEARCH_INDEX integrity: every SectionId has ≥1 entry (derived from NAV_GROUPS).
 *  8. SEARCH_INDEX integrity: all anchor values are non-empty strings.
 *  9. SEARCH_INDEX integrity: all id values are unique.
 * 10. SEARCH_INDEX integrity: all titleKey values are non-empty strings.
 * 11. SEARCH_INDEX integrity: manifest shape only (uniqueness/non-empty) —
 *     render-based anchor presence is covered by search-anchor.test.tsx.
 *
 * NOTE: The static INSTRUMENTED_ANCHORS Set check has been REMOVED. It was a
 * tautology (a Set checked against the same manifest catches no typos in the
 * component). The render-based guard in search-anchor.test.tsx catches those.
 */
import { describe, expect, it } from 'vitest';

import { NAV_GROUPS, type SectionId } from '@/features/settings/constants';
import { matchEntries } from '@/features/settings/lib/search';
import { SEARCH_INDEX } from '@/features/settings/lib/search-index';

// ── minimal TFunction stub ─────────────────────────────────────────────────────
// We only need to resolve a few keys for these unit tests.
const TRANSLATIONS: Record<string, string> = {
  'settings.sections.general.label': 'General',
  'settings.sections.appearance.label': 'Appearance',
  'settings.sections.contact.label': 'Contact Profile',
  'settings.sections.ai.label': 'AI',
  'settings.sections.jobs.label': 'Jobs',
  'settings.sections.resume.label': 'Resume',
  'settings.sections.accounts.label': 'Accounts',
  'settings.sections.privacy.label': 'Privacy',
  'settings.sections.performance.label': 'Performance',
  'settings.sections.developer.label': 'Developer',
  'settings.sections.about.label': 'Fund the hunt',
  // Entry title keys referenced by SEARCH_INDEX
  'settings.profile.title': 'Profile',
  'settings.language.title': 'Language',
  'settings.onboarding.title': 'Intro Wizard',
  'settings.startup.title': 'Startup',
  'settings.window.title': 'Window',
  'settings.update.title': 'Updates',
  'settings.appearance.scheme': 'Theme',
  'settings.appearance.accent': 'Accent color',
  'settings.appearance.textSize': 'Text size',
  'settings.appearance.reduceTransparency': 'Reduce transparency',
  'settings.appearance.increaseContrast': 'Increase contrast',
  'settings.contactProfile.title': 'Contact Profile',
  'settings.applicant.title': 'Applicant details',
  'settings.aiProvider.title': 'AI Provider',
  'settings.aiProvider.description': 'Choose where your AI runs',
  'settings.outputTone.title': 'AI Output Tone',
  'settings.companyResearch.title': 'Company Research',
  'settings.location.title': 'Preferred Location',
  'settings.techStack.title': 'Tech Stack',
  'settings.aggregatorKeys.title': 'Job search providers',
  'settings.resume.title': 'Resume Management',
  'settings.accounts.boardsTitle': 'Job board accounts',
  'settings.accounts.extension.title': 'Browser extension',
  'settings.privacy.dataTitle': 'Your data',
  'settings.privacy.resetApp': 'Reset App',
  'settings.performanceMode.heading': 'Performance Mode',
  'settings.developer.title': 'Developer Tools',
  'settings.about.title': 'Fund the hunt',
};

type TFunctionStub = (key: string) => string;
const t: TFunctionStub = (key: string) => TRANSLATIONS[key] ?? key;

/**
 * Derive SECTION_LABEL_KEYS from NAV_GROUPS (canonical source) instead of
 * duplicating a literal map. Any new section added to NAV_GROUPS is picked up
 * automatically and test coverage stays complete.
 */
const SECTION_LABEL_KEYS: Record<SectionId, string> = Object.fromEntries(
  NAV_GROUPS.flatMap((g) => g.items.map((item) => [item.id, item.label]))
) as Record<SectionId, string>;

// The stub satisfies the (key: string) => string signature matchEntries accepts.
const tFn = t;

// ── matchEntries ──────────────────────────────────────────────────────────────

describe('matchEntries', () => {
  it('returns [] for an empty query', () => {
    expect(matchEntries('', tFn, SECTION_LABEL_KEYS)).toHaveLength(0);
  });

  it('returns [] for a whitespace-only query', () => {
    expect(matchEntries('   ', tFn, SECTION_LABEL_KEYS)).toHaveLength(0);
  });

  it('matches on the localized title (case-insensitive) — general-profile entry is in results', () => {
    // "Profile" is the en title for 'settings.profile.title' — exactly one match expected
    const results = matchEntries('PROFILE', tFn, SECTION_LABEL_KEYS);
    // At least one result, and the specific 'general-profile' entry must be present
    expect(results.length).toBeGreaterThan(0);
    expect(results.map((r) => r.id)).toContain('general-profile');
    // The stub resolves 'settings.profile.title' → 'Profile' (from TRANSLATIONS map)
    const hit = results.find((r) => r.id === 'general-profile');
    expect(hit?.title).toBe('Profile');
    expect(hit?.section).toBe('general');
  });

  it('matches on a locale-invariant keyword — ai-provider entry is in results', () => {
    // 'ollama' is a keyword for ai-provider (and only ai-provider in the index)
    const results = matchEntries('ollama', tFn, SECTION_LABEL_KEYS);
    expect(results.length).toBeGreaterThan(0);
    expect(results.map((r) => r.id)).toContain('ai-provider');
    const hit = results.find((r) => r.id === 'ai-provider');
    expect(hit?.section).toBe('ai');
    expect(hit?.anchor).toBe('ai-provider');
  });

  it('matches on the localized section label — developer-tools entry is in results for "developer" query', () => {
    // 'Developer' is the en label for the developer section
    const results = matchEntries('developer', tFn, SECTION_LABEL_KEYS);
    expect(results.length).toBeGreaterThan(0);
    // The developer-tools entry must be present
    expect(results.map((r) => r.id)).toContain('developer-tools');
    const hit = results.find((r) => r.id === 'developer-tools');
    expect(hit?.section).toBe('developer');
  });

  it('returns [] when nothing matches (no-results path)', () => {
    const results = matchEntries('zzznomatchzzz', tFn, SECTION_LABEL_KEYS);
    expect(results).toHaveLength(0);
  });

  it('trims surrounding whitespace from the query before matching', () => {
    const trimmed = matchEntries('ollama', tFn, SECTION_LABEL_KEYS);
    const padded = matchEntries('  ollama  ', tFn, SECTION_LABEL_KEYS);
    expect(padded.map((r) => r.id)).toEqual(trimmed.map((r) => r.id));
  });

  it('each result carries a resolved title and sectionLabel', () => {
    const [result] = matchEntries('ollama', tFn, SECTION_LABEL_KEYS);
    if (!result) throw new Error('Expected at least one result');
    expect(typeof result.title).toBe('string');
    expect(result.title.length).toBeGreaterThan(0);
    expect(typeof result.sectionLabel).toBe('string');
    expect(result.sectionLabel.length).toBeGreaterThan(0);
  });
});

// ── SEARCH_INDEX integrity (manifest shape — render-based anchor guard is in search-anchor.test.tsx)

/**
 * Derive ALL_SECTION_IDS from NAV_GROUPS so a newly added section is
 * automatically included in coverage without any manual update here.
 */
const ALL_SECTION_IDS: SectionId[] = NAV_GROUPS.flatMap((g) => g.items.map((item) => item.id));

describe('SEARCH_INDEX integrity', () => {
  it('every SectionId (derived from NAV_GROUPS) has at least one entry', () => {
    for (const sectionId of ALL_SECTION_IDS) {
      const entries = SEARCH_INDEX.filter((e) => e.section === sectionId);
      expect(
        entries.length,
        `Section "${sectionId}" has no entries in SEARCH_INDEX`
      ).toBeGreaterThan(0);
    }
  });

  it('all anchor values are non-empty strings', () => {
    for (const entry of SEARCH_INDEX) {
      expect(
        typeof entry.anchor === 'string' && entry.anchor.trim().length > 0,
        `Entry "${entry.id}" has an empty anchor`
      ).toBe(true);
    }
  });

  it('all id values are unique', () => {
    const ids = SEARCH_INDEX.map((e) => e.id);
    const uniqueIds = new Set(ids);
    expect(uniqueIds.size, 'Duplicate id values found in SEARCH_INDEX').toBe(ids.length);
  });

  it('all titleKey values are non-empty strings', () => {
    for (const entry of SEARCH_INDEX) {
      expect(
        typeof entry.titleKey === 'string' && entry.titleKey.trim().length > 0,
        `Entry "${entry.id}" has an empty titleKey`
      ).toBe(true);
    }
  });

  it('all anchor values are unique (no duplicate anchors)', () => {
    const anchors = SEARCH_INDEX.map((e) => e.anchor);
    const uniqueAnchors = new Set(anchors);
    expect(uniqueAnchors.size, 'Duplicate anchor values found in SEARCH_INDEX').toBe(
      anchors.length
    );
  });

  // NOTE: The static INSTRUMENTED_ANCHORS Set check has been deliberately removed.
  // A string Set checked against the same manifest cannot catch typos in the
  // component that produces data-settings-anchor. That coverage lives in
  // search-anchor.test.tsx (render-based, catches actual DOM mismatches).
});
