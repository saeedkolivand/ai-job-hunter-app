/**
 * canonicalJobKey — the TS half of the cross-source dedup key (trust PR E,
 * stage 3). It is a LOCKSTEP mirror of the Rust `canonical_job_key`
 * (`scraping/boards/common.rs`) built on `normalize_job_url`
 * (`applications/mod.rs`).
 *
 * The first five cases copy the Rust truth-table inputs VERBATIM from
 * `scraping/boards/common/test.rs` (fixtures can't be literally shared across
 * languages, so duplicating the inputs is the drift guard: if either side
 * changes the algorithm, one of these — or its Rust twin — fails). The rest
 * pin TS-side URL-normalization details (Indeed `jk`, www/fragment/query/
 * trailing-slash, whole-URL lowercasing) that the Rust `normalize_job_url`
 * tests own on their side.
 */
import { describe, expect, it } from 'vitest';

import { canonicalJobKey } from './canonical-job-key';

// U+0001 (SOH) fallback-key separator — matches KEY_SEP in the impl and Rust's `\u{1}`.
const SEP = String.fromCharCode(1);

describe('canonicalJobKey — Rust truth-table parity (verbatim inputs)', () => {
  it('same URL across boards collapses (www/tracking variants, differing titles)', () => {
    const a = canonicalJobKey(
      'https://www.acme.example/jobs/42?utm_source=x',
      'Senior Engineer',
      'Acme'
    );
    const b = canonicalJobKey('https://acme.example/jobs/42', 'Sr. Engineer', 'Acme Inc');
    expect(a).toBe(b);
    expect(a).toBe('https://acme.example/jobs/42');
  });

  it('URL-less rows match on normalized title + company', () => {
    const a = canonicalJobKey('', ' Senior Rust Engineer ', 'Acme');
    const b = canonicalJobKey('', 'senior rust engineer', '  ACME ');
    expect(a).toBe(b);
    expect(a).toBe(`senior rust engineer${SEP}acme`);
  });

  it('URL-less non-ASCII title+company normalizes to the exact expected key (Rust drift guard)', () => {
    // Same case verbatim is pinned in the Rust truth table — a JS `.toLowerCase()`
    // vs Rust `.to_lowercase()` divergence on accented Unicode would show up here
    // as a mismatched exact string, not just an equality-between-variants check.
    const key = canonicalJobKey('', 'Développeur Sénior', 'Müller GmbH');
    expect(key).toBe(`développeur sénior${SEP}müller gmbh`);
  });

  it('near-miss titles at the same company stay distinct', () => {
    const senior = canonicalJobKey('', 'Senior Rust Engineer', 'Acme');
    const plain = canonicalJobKey('', 'Rust Engineer', 'Acme');
    expect(senior).not.toBe(plain);
  });

  it('the SOH separator cannot be forged by a title containing the company', () => {
    const forged = canonicalJobKey('', 'Engineer Acme', '');
    const genuine = canonicalJobKey('', 'Engineer', 'Acme');
    expect(forged).not.toBe(genuine);
  });

  it('empty / whitespace / non-http(s) schemes fall back to title+company', () => {
    const expected = `t${SEP}co`;
    expect(canonicalJobKey('', 'T', 'Co')).toBe(expected);
    expect(canonicalJobKey('   ', 'T', 'Co')).toBe(expected);
    expect(canonicalJobKey('javascript:alert(1)', 'T', 'Co')).toBe(expected);
    expect(canonicalJobKey('file:///etc/passwd', 'T', 'Co')).toBe(expected);
  });
});

describe('canonicalJobKey — URL normalization details (mirror normalize_job_url)', () => {
  it('keeps Indeed jk, drops other query params, strips www', () => {
    expect(
      canonicalJobKey('https://www.indeed.com/viewjob?jk=abc123&utm_source=x', 'Engineer', 'Acme')
    ).toBe('https://indeed.com/viewjob?jk=abc123');
  });

  it('keeps jk on Indeed subdomains and is param-order independent', () => {
    const a = canonicalJobKey('https://de.indeed.com/viewjob?jk=abc123', 'Engineer', 'Acme');
    const b = canonicalJobKey(
      'https://de.indeed.com/viewjob?utm_source=x&jk=abc123',
      'Engineer',
      'Acme'
    );
    expect(a).toBe(b);
    expect(a).toBe('https://de.indeed.com/viewjob?jk=abc123');
  });

  it('drops jk for non-Indeed hosts (jk is not identifying there)', () => {
    expect(canonicalJobKey('https://acme.example/jobs?jk=x', 'Engineer', 'Acme')).toBe(
      'https://acme.example/jobs'
    );
  });

  it('drops the fragment and strips a trailing slash', () => {
    expect(canonicalJobKey('https://www.acme.example/jobs/42/#apply', 'Engineer', 'Acme')).toBe(
      'https://acme.example/jobs/42'
    );
  });

  it('lowercases the WHOLE url including the path (byte-equivalent to Rust, unlike URL())', () => {
    expect(canonicalJobKey('https://Acme.EXAMPLE/Jobs/ABC', 'Engineer', 'Acme')).toBe(
      'https://acme.example/jobs/abc'
    );
  });

  it('keys a scheme-less url by the url itself (not the title fallback)', () => {
    expect(canonicalJobKey('acme.example/jobs/42', 'Engineer', 'Acme')).toBe(
      'acme.example/jobs/42'
    );
  });

  it('a malformed URL (empty authority) does not throw and returns deterministic output', () => {
    // "https:///path" has a scheme but an empty host — no `URL`/regex parse step
    // exists here to throw on it (raw string mirror, by design); this pins that
    // the fn stays a pure no-crash string transform on odd input, same as Rust's
    // `normalize_job_url` (no panics, no `Result`).
    expect(() => canonicalJobKey('https:///path', 'Engineer', 'Acme')).not.toThrow();
    const key = canonicalJobKey('https:///path', 'Engineer', 'Acme');
    expect(typeof key).toBe('string');
    // Same input twice must key identically (determinism, not just no-crash).
    expect(canonicalJobKey('https:///path', 'Engineer', 'Acme')).toBe(key);
  });

  it('keeps distinct Hacker News job ids apart', () => {
    // `boards::ycombinator` builds `news.ycombinator.com/item?id=<id>` when an HN
    // job story has no external link, so the id lives in the query — which is
    // dropped for any host without an allowlist entry.
    const a = canonicalJobKey('https://news.ycombinator.com/item?id=44100001', 'BE', 'Acme');
    const b = canonicalJobKey('https://news.ycombinator.com/item?id=44100002', 'FE', 'Beta');
    expect(a).not.toBe(b);
    expect(a).toBe('https://news.ycombinator.com/item?id=44100001');

    // Tracking params on the same id still collapse, in either order.
    expect(
      canonicalJobKey('https://news.ycombinator.com/item?utm_source=x&id=44100001', 'BE', 'Acme')
    ).toBe(a);
  });
});
