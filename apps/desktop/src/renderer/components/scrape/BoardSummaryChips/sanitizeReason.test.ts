/**
 * sanitizeReason (pure): paths / URLs / host:port / email / credential are
 * redacted; ordinary message text survives; length is capped.
 */

import { describe, expect, it } from 'vitest';

import { sanitizeReason } from './sanitizeReason';

describe('sanitizeReason', () => {
  it('keeps an ordinary status message intact', () => {
    expect(sanitizeReason('429 Too Many Requests')).toBe('429 Too Many Requests');
  });

  it('redacts a Windows absolute path but keeps the surrounding message', () => {
    const out = sanitizeReason('failed to read C:\\Users\\alice\\creds.json');
    expect(out).toContain('failed to read');
    expect(out).toContain('<path-redacted>');
    expect(out).not.toMatch(/alice/i);
  });

  it('redacts a Unix absolute path', () => {
    expect(sanitizeReason('open /etc/passwd denied')).toContain('<path-redacted>');
  });

  it('redacts a drive-less home path', () => {
    expect(sanitizeReason('at Users/bob/app')).toContain('<path-redacted>');
  });

  it('redacts a full URL (with query string)', () => {
    const out = sanitizeReason('GET https://api.example.com/v1?app_key=sekret failed');
    expect(out).toContain('<url-redacted>');
    expect(out).not.toContain('sekret');
  });

  it('redacts a bare host:port and a dotted IPv4', () => {
    expect(sanitizeReason('refused api.example.com:8080')).toContain('<host-redacted>');
    expect(sanitizeReason('connect 192.168.0.1')).toContain('<host-redacted>');
  });

  it('redacts an email address', () => {
    expect(sanitizeReason('user alice@example.com blocked')).toContain('<email-redacted>');
  });

  it('redacts a standalone credential assignment', () => {
    expect(sanitizeReason('bad token=abc123def')).toContain('<credential-redacted>');
  });

  it('caps the length and appends an ellipsis', () => {
    const out = sanitizeReason('x'.repeat(400));
    expect(out.length).toBeLessThanOrEqual(201);
    expect(out.endsWith('…')).toBe(true);
  });

  it('returns empty string for a non-string input', () => {
    expect(sanitizeReason(undefined as unknown as string)).toBe('');
  });

  it('does NOT redact a ratio-shaped token like "3.5:1" (no hostname-like pre-colon part)', () => {
    expect(sanitizeReason('contrast ratio 3.5:1 too low')).toBe('contrast ratio 3.5:1 too low');
  });

  it('still redacts a real host:port with a lettered hostname', () => {
    expect(sanitizeReason('refused api.example.com:8080')).toContain('<host-redacted>');
  });

  it('strips a trailing ")" before classifying a parenthesized path', () => {
    const out = sanitizeReason('failed (C:\\Users\\alice\\creds.json)');
    expect(out).toContain('<path-redacted>');
    expect(out).not.toMatch(/alice/i);
  });

  it('redacts a UNC network path (\\\\server\\share\\...)', () => {
    const out = sanitizeReason('failed to read \\\\fileserver01\\shared\\secrets.json');
    expect(out).toContain('failed to read');
    expect(out).toContain('<path-redacted>');
    expect(out).not.toMatch(/fileserver01/i);
  });

  it('pre-caps a pathological (very long) input before tokenizing, doing bounded work', () => {
    // 50,000 chars with no whitespace — a single giant "token". Must not hang
    // or blow the call stack; the 1000-char input pre-cap bounds the work
    // regardless of the eventual MAX_REASON_LEN output truncation.
    const pathological = 'a'.repeat(50_000);
    const out = sanitizeReason(pathological);
    expect(out.length).toBeLessThanOrEqual(201);
    expect(out.endsWith('…')).toBe(true);
  });
});
