/**
 * Global en/de key-set + placeholder parity for @ajh/translations.
 *
 * The per-feature `*.i18n.test.ts` files assert `t(key) !== key`, which does
 * NOT catch a key missing in one locale — @ajh/translations initializes with
 * `fallbackLng: 'en'` (packages/translations/src/index.ts), so a missing DE
 * key silently resolves to the ENGLISH string instead of the raw key. This
 * test reads the raw resource trees directly (bypassing the fallback chain
 * entirely) so a locale gap is caught regardless of what `t()` would return.
 */

import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

/** A nested translation resource tree: leaves (string or otherwise), arrays, or further nesting. */
type ResourceTree = { [key: string]: unknown };

/**
 * Flatten a nested resource tree to a Map of dot-path -> leaf value (stringified).
 * Leaf classification is exhaustive: any non-object value (string, number,
 * boolean, null) is recorded via `String(value)` so a cross-locale TYPE
 * mismatch (e.g. `"count": 5` vs `"count": "5"`) is still caught by the
 * key-set/placeholder checks below instead of silently vanishing.
 */
export function flatten(tree: ResourceTree, prefix = ''): Map<string, string> {
  const out = new Map<string, string>();
  for (const [key, value] of Object.entries(tree)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === 'object') {
      for (const [k, v] of flatten(value as ResourceTree, path)) out.set(k, v);
    } else {
      out.set(path, String(value));
    }
  }
  return out;
}

/** Keys present in `a` but not `b`, sorted. */
export function keysOnlyIn(a: Map<string, string>, b: Map<string, string>): string[] {
  return [...a.keys()].filter((k) => !b.has(k)).sort();
}

/** `{{name}}` interpolation placeholder names referenced in a translation string. */
export function extractPlaceholders(value: string): Set<string> {
  const out = new Set<string>();
  for (const match of value.matchAll(/\{\{\s*([\w-]+)[^}]*\}\}/g)) {
    const name = match[1];
    if (name) out.add(name);
  }
  return out;
}

function setsEqual(a: Set<string>, b: Set<string>): boolean {
  if (a.size !== b.size) return false;
  for (const v of a) if (!b.has(v)) return false;
  return true;
}

describe('flatten / keysOnlyIn / extractPlaceholders — self-check', () => {
  it('flatten produces dot-path leaf keys through nesting and arrays', () => {
    const tree: ResourceTree = {
      a: 'x',
      b: { c: 'y', d: { e: 'z' } },
      f: ['p', 'q'],
    };
    const flat = flatten(tree);
    expect([...flat.keys()].sort()).toEqual(['a', 'b.c', 'b.d.e', 'f.0', 'f.1']);
    expect(flat.get('b.d.e')).toBe('z');
  });

  it('keysOnlyIn reports a deliberately missing key without mutating real resources', () => {
    const en = flatten({ shared: 'ok', onlyEn: 'english only' });
    const de = flatten({ shared: 'ok' });
    expect(keysOnlyIn(en, de)).toEqual(['onlyEn']);
    expect(keysOnlyIn(de, en)).toEqual([]);
  });

  it('extractPlaceholders finds interpolation names and ignores plain text', () => {
    expect([...extractPlaceholders('Hi {{name}}, you have {{count}} items')].sort()).toEqual([
      'count',
      'name',
    ]);
    expect([...extractPlaceholders('no placeholders here')]).toEqual([]);
  });

  it('a placeholder-name mismatch between two synthetic strings is detectable', () => {
    const enPlaceholders = extractPlaceholders('{{count}} results');
    const dePlaceholders = extractPlaceholders('{{anzahl}} Ergebnisse');
    expect(setsEqual(enPlaceholders, dePlaceholders)).toBe(false);
  });

  it('a non-string leaf no longer vanishes from the map, so a numeric-vs-string gap is still reported', () => {
    // Regression for a fixed bug: a number/boolean/null leaf used to fall
    // through flatten's classification silently, so a key present only as
    // `en.count: 5` never entered the map at all — keysOnlyIn had nothing to
    // compare and the gap escaped the guard entirely.
    const en = flatten({ shared: 'ok', count: 5 }); // numeric leaf, en-only
    const de = flatten({ shared: 'ok' }); // de is genuinely missing this key
    expect(en.get('count')).toBe('5'); // captured, not silently dropped
    expect(keysOnlyIn(en, de)).toEqual(['count']); // now reported as missing in de
  });
});

describe('en/de translation resources — key-set parity', () => {
  const en = flatten(i18n.getResourceBundle('en', 'translation') as ResourceTree);
  const de = flatten(i18n.getResourceBundle('de', 'translation') as ResourceTree);

  it('has non-trivial resource bundles loaded (sanity)', () => {
    expect(en.size).toBeGreaterThan(100);
    expect(de.size).toBeGreaterThan(100);
  });

  it('every en key exists in de', () => {
    const missingInDe = keysOnlyIn(en, de);
    expect(missingInDe, `keys present in en but missing in de:\n${missingInDe.join('\n')}`).toEqual(
      []
    );
  });

  it('every de key exists in en', () => {
    const missingInEn = keysOnlyIn(de, en);
    expect(missingInEn, `keys present in de but missing in en:\n${missingInEn.join('\n')}`).toEqual(
      []
    );
  });

  it('shared keys interpolate the same {{placeholder}} names in both locales', () => {
    const mismatches: string[] = [];
    for (const [key, enValue] of en) {
      const deValue = de.get(key);
      if (deValue === undefined) continue; // reported by the key-set parity tests above
      const enPlaceholders = extractPlaceholders(enValue);
      const dePlaceholders = extractPlaceholders(deValue);
      if (!setsEqual(enPlaceholders, dePlaceholders)) {
        mismatches.push(
          `${key}: en={${[...enPlaceholders].sort().join(', ')}} de={${[...dePlaceholders]
            .sort()
            .join(', ')}}`
        );
      }
    }
    expect(mismatches, `placeholder mismatches:\n${mismatches.join('\n')}`).toEqual([]);
  });
});
