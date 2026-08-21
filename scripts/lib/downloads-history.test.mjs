import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { mergePoints, readJsonArray } from './downloads-history.mjs';

/** Write `body` to a scratch file and return its path. */
function fixture(body) {
  return (() => {
    const p = join(mkdtempSync(join(tmpdir(), 'dl-history-')), 'history.json');
    writeFileSync(p, body);
    return p;
  })();
}

describe('readJsonArray', () => {
  it('returns the parsed array for a well-formed file', () => {
    const path = fixture('[{"date":"2026-07-21","value":268}]');
    expect(readJsonArray(path, 'history')).toEqual([{ date: '2026-07-21', value: 268 }]);
  });

  // The whole reason this function is not forgiving. The publish target is a
  // parentless, force-pushed branch, so a silent [] here overwrites the only
  // copy of every reading since the seed. It has to throw, not degrade.
  it('throws on malformed JSON rather than degrading to an empty series', () => {
    const path = fixture('not json{');
    expect(() => readJsonArray(path, 'prior downloads history')).toThrow();
  });

  it('throws when the JSON is valid but is not an array', () => {
    const path = fixture('{"oops":1}');
    expect(() => readJsonArray(path, 'prior downloads history')).toThrow(TypeError);
  });

  it('throws when the file does not exist', () => {
    expect(() => readJsonArray(join(tmpdir(), 'definitely-absent-history.json'), 'x')).toThrow();
  });
});

describe('mergePoints', () => {
  it('sorts by date and de-duplicates', () => {
    expect(
      mergePoints([{ date: '2026-08-02', value: 5 }], [{ date: '2026-08-01', value: 3 }])
    ).toEqual([
      { date: '2026-08-01', value: 3 },
      { date: '2026-08-02', value: 5 },
    ]);
  });

  // A download count only ever grows, so the larger reading for a date is the
  // later observation. Asserted against an absolute expected value, not against
  // another call's output, so a regression that lowered both would still fail.
  it('keeps the highest reading for a date, whichever list it came from', () => {
    const older = [{ date: '2026-08-01', value: 400 }];
    const newer = [{ date: '2026-08-01', value: 465 }];
    expect(mergePoints(older, newer)).toEqual([{ date: '2026-08-01', value: 465 }]);
    // Order-independent: swapping the arguments must not change the result.
    expect(mergePoints(newer, older)).toEqual([{ date: '2026-08-01', value: 465 }]);
  });

  it('skips entries with no date or a non-numeric value', () => {
    expect(
      mergePoints([
        { date: '2026-08-01', value: 10 },
        { date: '', value: 99 },
        { date: '2026-08-03', value: '12' },
        null,
      ])
    ).toEqual([{ date: '2026-08-01', value: 10 }]);
  });

  it('tolerates absent lists', () => {
    expect(mergePoints(undefined, [{ date: '2026-08-01', value: 1 }], null)).toEqual([
      { date: '2026-08-01', value: 1 },
    ]);
  });
});
