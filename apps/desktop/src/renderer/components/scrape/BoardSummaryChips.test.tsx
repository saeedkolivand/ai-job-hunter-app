/**
 * BoardSummaryChips — per-board scrape diagnostics strip.
 *
 * Covers:
 *  - sanitizeReason (pure): paths / URLs / host:port / email / credential are
 *    redacted; ordinary message text survives; length is capped.
 *  - Chip variants: success (count, green), error (red, sanitized reason),
 *    skipped (neutral "default", mapped reason), truncated (amber "partial").
 *  - Per-board precedence: error > skipped > truncated > success.
 *  - Unknown-shape tolerance: malformed/legacy entries are dropped, not trusted.
 *
 * @ajh/ui `Tag` is stubbed to surface its `color` prop as `data-color` so each
 * chip's tone is assertable; @ajh/translations is a readable identity mock.
 */

import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { BoardScrapeSummary } from '@ajh/shared';

import { BoardSummaryChips, sanitizeReason } from './BoardSummaryChips';

// ── @ajh/translations — readable identity mock ────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, opts?: Record<string, unknown>) => {
      if (opts && 'count' in opts) return `${k}:${String(opts.count)}`;
      if (opts && 'defaultValue' in opts) return `label(${String(opts.defaultValue)})`;
      return k;
    },
  }),
}));

// ── @ajh/ui — expose Tag color + className so tone AND wrap classes are
// assertable ────────────────────────────────────────────────────────────────

vi.mock('@ajh/ui', () => ({
  Tag: ({
    color,
    className,
    children,
  }: {
    color?: string;
    className?: string;
    children?: React.ReactNode;
  }) => (
    <span data-testid="chip" data-color={color} className={className}>
      {children}
    </span>
  ),
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

function chips() {
  return screen.queryAllByTestId('chip');
}

// ─────────────────────────────────────────────────────────────────────────────
// sanitizeReason
// ─────────────────────────────────────────────────────────────────────────────

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
});

// ─────────────────────────────────────────────────────────────────────────────
// Chip variants
// ─────────────────────────────────────────────────────────────────────────────

describe('BoardSummaryChips — variants', () => {
  it('success board renders a green count chip', () => {
    render(<BoardSummaryChips summaries={[{ board: 'greenhouse', count: 12 }]} />);
    const chip = chips()[0];
    expect(chip?.getAttribute('data-color')).toBe('success');
    expect(chip?.textContent).toContain('label(greenhouse)');
    expect(chip?.textContent).toContain('jobs.boardSummary.count:12');
  });

  it('errored board renders a red chip with the sanitized reason', () => {
    render(
      <BoardSummaryChips
        summaries={[{ board: 'linkedin', count: 0, error: 'blocked at C:\\Users\\me\\x' }]}
      />
    );
    const chip = chips()[0];
    expect(chip?.getAttribute('data-color')).toBe('error');
    expect(chip?.textContent).toContain('blocked at');
    expect(chip?.textContent).toContain('<path-redacted>');
    expect(chip?.textContent).not.toMatch(/Users/);
  });

  it('skipped board renders a neutral chip with a mapped reason (never the raw enum)', () => {
    render(
      <BoardSummaryChips summaries={[{ board: 'aggregator', count: 0, skipped: 'needs-keys' }]} />
    );
    const chip = chips()[0];
    expect(chip?.getAttribute('data-color')).toBe('default');
    expect(chip?.textContent).toContain('jobs.boardSummary.skip.needsKeys');
    // The raw enum value must not be rendered.
    expect(chip?.textContent).not.toContain('needs-keys');
  });

  it('truncated board renders an amber "partial" chip and never leaks the reason text', () => {
    render(
      <BoardSummaryChips
        summaries={[{ board: 'lever', count: 8, truncated: 'page 3 of 5 failed: HTTP 429' }]}
      />
    );
    const chip = chips()[0];
    expect(chip?.getAttribute('data-color')).toBe('warning');
    expect(chip?.textContent).toContain('jobs.boardSummary.partial');
    expect(chip?.textContent).not.toContain('page 3');
  });

  it('per-board precedence: error wins over a co-present skip', () => {
    render(
      <BoardSummaryChips
        summaries={[{ board: 'x', count: 0, error: 'boom', skipped: 'needs-login' }]}
      />
    );
    expect(chips()[0]?.getAttribute('data-color')).toBe('error');
  });

  it('unknown skipped reason falls back to the generic label (no raw string)', () => {
    render(
      <BoardSummaryChips
        summaries={
          [{ board: 'x', count: 0, skipped: 'mystery' }] as unknown as BoardScrapeSummary[]
        }
      />
    );
    expect(chips()[0]?.textContent).toContain('jobs.boardSummary.skip.other');
    expect(chips()[0]?.textContent).not.toContain('mystery');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Robustness
// ─────────────────────────────────────────────────────────────────────────────

describe('BoardSummaryChips — robustness', () => {
  it('renders nothing for an empty array', () => {
    render(<BoardSummaryChips summaries={[]} />);
    expect(screen.queryByRole('group')).toBeNull();
    expect(chips()).toHaveLength(0);
  });

  it('tolerates unknown/malformed shapes, keeping only well-formed entries', () => {
    // The second survivor carries an error so the all-ok collapse (below)
    // doesn't fold both survivors into one chip — keeps this test focused on
    // shape tolerance, not the collapse behavior.
    const summaries = [
      null,
      {},
      { board: '' },
      { board: 'ok', count: 'nope' },
      'str',
      { board: 'greenhouse', count: 0, error: 'boom' },
    ] as unknown as BoardScrapeSummary[];
    render(<BoardSummaryChips summaries={summaries} />);
    // Only { board: 'ok' } (count coerced to 0) and { board: 'greenhouse' } survive.
    expect(chips()).toHaveLength(2);
    expect(chips()[0]?.textContent).toContain('label(ok)');
    expect(chips()[0]?.textContent).toContain('jobs.boardSummary.count:0');
  });

  it('exposes an accessible group with a localized label', () => {
    render(<BoardSummaryChips summaries={[{ board: 'greenhouse', count: 1 }]} />);
    expect(screen.getByRole('group')).toHaveAttribute('aria-label', 'jobs.boardSummary.label');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// All-ok collapse (ui-ux Q1) + chip-detail cap + wrap-safe className
// ─────────────────────────────────────────────────────────────────────────────

describe('BoardSummaryChips — all-ok collapse', () => {
  it('collapses to ONE chip when every board succeeded (2+ boards)', () => {
    render(
      <BoardSummaryChips
        summaries={[
          { board: 'greenhouse', count: 4 },
          { board: 'lever', count: 2 },
        ]}
      />
    );
    expect(chips()).toHaveLength(1);
    expect(chips()[0]?.getAttribute('data-color')).toBe('success');
    expect(chips()[0]?.textContent).toBe('jobs.boardSummary.allOk:2');
  });

  it('does NOT collapse a single successful board', () => {
    render(<BoardSummaryChips summaries={[{ board: 'greenhouse', count: 4 }]} />);
    expect(chips()).toHaveLength(1);
    expect(chips()[0]?.textContent).toContain('label(greenhouse)');
  });

  it('does NOT collapse when any board is non-success', () => {
    render(
      <BoardSummaryChips
        summaries={[
          { board: 'greenhouse', count: 4 },
          { board: 'linkedin', count: 0, error: 'blocked' },
        ]}
      />
    );
    expect(chips()).toHaveLength(2);
  });
});

describe('BoardSummaryChips — chip detail cap + wrap classes', () => {
  it('caps a long error reason for display, distinct from the 200-char sanitize ceiling', () => {
    const longError = `network failure ${'x'.repeat(120)} while fetching`;
    render(<BoardSummaryChips summaries={[{ board: 'x', count: 0, error: longError }]} />);
    const text = chips()[0]?.textContent ?? '';
    // "label(x)· " prefix + capped detail (<=60 chars + ellipsis).
    const detail = text.split('· ')[1] ?? '';
    expect(detail.length).toBeLessThanOrEqual(61);
    expect(detail.endsWith('…')).toBe(true);
  });

  it('the Tag className allows wrapping instead of forcing single-line overflow', () => {
    render(<BoardSummaryChips summaries={[{ board: 'greenhouse', count: 4, error: 'boom' }]} />);
    const cls = chips()[0]?.className ?? '';
    expect(cls).toContain('whitespace-normal');
    expect(cls).toContain('break-words');
    expect(cls).not.toContain('opacity-75');
  });
});
