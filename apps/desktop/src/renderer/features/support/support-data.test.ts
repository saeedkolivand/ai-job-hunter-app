/**
 * Help corpus ↔ translations parity (ADR-041).
 *
 * `getSupportSections` reaches every string through `t()`, so a missing key
 * renders as the raw dotted key — visible, but only to whoever opens the page
 * in that language. This test drives the corpus with a recording `t` and
 * resolves each key it emits against BOTH shipped bundles, so an entry added
 * to `en` only (the usual half-translation) fails here instead of shipping.
 *
 * The `i18next-cli` extractor step covers the same ground but is advisory in
 * CI (`continue-on-error`), so this file is the part that actually blocks.
 *
 * The bundles are imported as JSON rather than read through `@ajh/translations`
 * on purpose: that entrypoint initializes i18next with `fallbackLng: 'en'`, so
 * a `de` lookup for a missing key returns the English string and every
 * assertion below would pass on a bundle that is missing the key entirely.
 */
import { describe, expect, it } from 'vitest';

import { getSupportSections } from '@/features/support/support-data';

import de from '../../../../../../packages/translations/src/locales/de/translation.json';
import en from '../../../../../../packages/translations/src/locales/en/translation.json';

const BUNDLES: Record<string, unknown> = { en, de };

/** Keys `SupportPage` and `HelpChat` pass to `t()` themselves — invisible to
 *  the corpus walk. */
const PAGE_KEYS = [
  'support.faq.title',
  'support.faq.subtitle',
  'support.faq.badge',
  'support.search.placeholder',
  'support.search.ariaLabel',
  'support.search.noResultsTitle',
  'support.search.noResultsBody',
  // i18next resolves `t('…resultCount', { count })` to one of these two.
  'support.search.resultCount_one',
  'support.search.resultCount_other',
  // The help chat card (ADR-043).
  'support.chat.title',
  'support.chat.subtitle',
  'support.chat.placeholder',
  'support.chat.ariaLabel',
  'support.chat.ask',
  'support.chat.stop',
  'support.chat.thinking',
  'support.chat.you',
  'support.chat.assistant',
  'support.chat.basedOn',
  'support.chat.sourceHint',
  'support.chat.caption',
  'support.chat.keywordNotice',
  'support.chat.denseUnavailable',
  'support.chat.keywordAction',
  'support.chat.error',
  'support.chat.retry',
  // i18next resolves `t('…charsLeft', { count })` to one of these two.
  'support.chat.charsLeft_one',
  'support.chat.charsLeft_other',
];

/** Walks a dotted key through a bundle; `undefined` when a segment is missing. */
function lookup(bundle: unknown, key: string): unknown {
  return key
    .split('.')
    .reduce<unknown>(
      (node, segment) =>
        typeof node === 'object' && node !== null
          ? (node as Record<string, unknown>)[segment]
          : undefined,
      bundle
    );
}

/** `"<locale>: <key>"` for every key that is missing or blank. */
function unresolved(keys: readonly string[]): string[] {
  const missing: string[] = [];
  for (const [locale, bundle] of Object.entries(BUNDLES)) {
    for (const key of keys) {
      const value = lookup(bundle, key);
      if (typeof value !== 'string' || value.trim() === '') missing.push(`${locale}: ${key}`);
    }
  }
  return missing;
}

const recorded: string[] = [];
const sections = getSupportSections((key) => {
  recorded.push(key);
  return key;
});

describe('support corpus / translations parity', () => {
  it('emits one key per label, question and answer', () => {
    // Absolute floor first: a corpus that returned nothing would otherwise
    // satisfy every "no missing key" assertion below vacuously.
    expect(recorded.length).toBeGreaterThan(50);

    // Then the shape: one key per section label plus two per entry. Short of
    // this and some string is hardcoded rather than translated.
    const expected =
      sections.length + sections.reduce((n, section) => n + section.problems.length * 2, 0);
    expect(recorded).toHaveLength(expected);
  });

  it('resolves every corpus key to a non-empty string in en AND de', () => {
    expect(unresolved(recorded)).toEqual([]);
  });

  it('resolves every key SupportPage uses itself in en AND de', () => {
    expect(unresolved(PAGE_KEYS)).toEqual([]);
  });

  it('gives every entry a GLOBALLY unique id', () => {
    // The per-section check below is about React list keys. This one is about
    // the wire: `help:search` keys its reply by entry id ALONE, and
    // `use-help-chat` maps those ids back to entry text through one flat Map.
    // Two sections sharing a leaf id would silently attribute an answer to the
    // wrong entry — and the "Based on" chip would search for the wrong topic.
    const ids = sections.flatMap((section) => section.problems.map((p) => p.id));
    const duplicated = ids.filter((id, i) => ids.indexOf(id) !== i);
    expect(duplicated).toEqual([]);
  });

  it('ties every section id to the translation namespace its entries live under', () => {
    // `Section.id` is not decoration: `use-help-chat` decides whether the data
    // glance may name the user's applications by comparing it to
    // `'applications'`. If an id drifted from its `support.faq.<id>Questions.*`
    // namespace, that comparison would silently stop matching and the check
    // would look like it still worked.
    const keys = sections.flatMap((section) =>
      section.problems.map((p) => `support.faq.${section.id}Questions.${p.id}.q`)
    );
    expect(unresolved(keys)).toEqual([]);
    // And the section the privacy gate names must actually exist.
    expect(sections.map((section) => section.id)).toContain('applications');
  });

  it('gives every entry a unique id within its section', () => {
    // The id is the React list key for the accordions, so a duplicate silently
    // drops an entry from the rendered list.
    const duplicated = sections.flatMap((section) => {
      const ids = section.problems.map((p) => p.id);
      return ids.filter((id, i) => ids.indexOf(id) !== i).map((id) => `${section.label}: ${id}`);
    });
    expect(duplicated).toEqual([]);
  });
});
