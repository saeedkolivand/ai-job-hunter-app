/**
 * BoardSummaryChips — chip variants, precedence, robustness, all-ok collapse.
 *
 *  - Variants: success (count, green), error (red, sanitized reason),
 *    skipped (neutral "default", mapped reason), truncated (amber "partial").
 *  - Per-board precedence: error > skipped > truncated > success.
 *  - Unknown-shape tolerance: malformed/legacy entries are dropped, not trusted.
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { BoardScrapeSummary } from '@ajh/shared';

import { BoardSummaryChips } from '../BoardSummaryChips';
import { chips, renderChips, renderFirstChip } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-mocks')).translationsMock());
vi.mock('@ajh/ui', async () => (await import('./test-mocks')).uiMock());

describe('BoardSummaryChips — variants', () => {
  it('success board renders a green count chip', () => {
    const chip = renderFirstChip({ board: 'greenhouse', count: 12 });
    expect(chip?.getAttribute('data-color')).toBe('success');
    expect(chip?.textContent).toContain('label(greenhouse)');
    expect(chip?.textContent).toContain('jobs.boardSummary.count:12');
  });

  it('errored board renders a red chip with the sanitized reason', () => {
    const chip = renderFirstChip({
      board: 'linkedin',
      count: 0,
      error: 'blocked at C:\\Users\\me\\x',
    });
    expect(chip?.getAttribute('data-color')).toBe('error');
    expect(chip?.textContent).toContain('blocked at');
    expect(chip?.textContent).toContain('<path-redacted>');
    expect(chip?.textContent).not.toMatch(/Users/);
  });

  it('skipped board renders a neutral chip with a mapped reason (never the raw enum)', () => {
    const chip = renderFirstChip({ board: 'aggregator', count: 0, skipped: 'needs-keys' });
    expect(chip?.getAttribute('data-color')).toBe('default');
    expect(chip?.textContent).toContain('jobs.boardSummary.skip.needsKeys');
    // The raw enum value must not be rendered.
    expect(chip?.textContent).not.toContain('needs-keys');
  });

  it('truncated board renders an amber "partial" chip and never leaks the reason text', () => {
    const chip = renderFirstChip({
      board: 'lever',
      count: 8,
      truncated: 'page 3 of 5 failed: HTTP 429',
    });
    expect(chip?.getAttribute('data-color')).toBe('warning');
    expect(chip?.textContent).toContain('jobs.boardSummary.partial');
    expect(chip?.textContent).not.toContain('page 3');
  });

  it('per-board precedence: error wins over a co-present skip', () => {
    const chip = renderFirstChip({ board: 'x', count: 0, error: 'boom', skipped: 'needs-login' });
    expect(chip?.getAttribute('data-color')).toBe('error');
  });

  it('unknown skipped reason falls back to the generic label (no raw string)', () => {
    const chip = renderFirstChip({
      board: 'x',
      count: 0,
      skipped: 'mystery',
    } as unknown as BoardScrapeSummary);
    expect(chip?.textContent).toContain('jobs.boardSummary.skip.other');
    expect(chip?.textContent).not.toContain('mystery');
  });
});

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
    const all = renderChips([
      null,
      {},
      { board: '' },
      { board: 'ok', count: 'nope' },
      'str',
      { board: 'greenhouse', count: 0, error: 'boom' },
    ] as unknown as BoardScrapeSummary[]);
    // Only { board: 'ok' } (count coerced to 0) and { board: 'greenhouse' } survive.
    expect(all).toHaveLength(2);
    expect(all[0]?.textContent).toContain('label(ok)');
    expect(all[0]?.textContent).toContain('jobs.boardSummary.count:0');
  });

  it('exposes an accessible group with a localized label', () => {
    renderChips([{ board: 'greenhouse', count: 1 }]);
    expect(screen.getByRole('group')).toHaveAttribute('aria-label', 'jobs.boardSummary.label');
  });
});

describe('BoardSummaryChips — all-ok collapse', () => {
  it('collapses to ONE chip when every board succeeded (2+ boards)', () => {
    const all = renderChips([
      { board: 'greenhouse', count: 4 },
      { board: 'lever', count: 2 },
    ]);
    expect(all).toHaveLength(1);
    expect(all[0]?.getAttribute('data-color')).toBe('success');
    expect(all[0]?.textContent).toBe('jobs.boardSummary.allOk:2');
  });

  it('does NOT collapse a single successful board', () => {
    const all = renderChips([{ board: 'greenhouse', count: 4 }]);
    expect(all).toHaveLength(1);
    expect(all[0]?.textContent).toContain('label(greenhouse)');
  });

  it('does NOT collapse when any board is non-success', () => {
    const all = renderChips([
      { board: 'greenhouse', count: 4 },
      { board: 'linkedin', count: 0, error: 'blocked' },
    ]);
    expect(all).toHaveLength(2);
  });

  it('does NOT collapse when a sibling board carries an informational note (note ≠ success)', () => {
    const all = renderChips([
      { board: 'a', count: 4 },
      { board: 'b', count: 2, notes: ['broadened:de'] },
    ]);
    expect(all).toHaveLength(2);
    expect(all[1]?.getAttribute('data-color')).toBe('processing');
  });
});

describe('BoardSummaryChips — chip detail cap + wrap classes', () => {
  it('caps a long error reason for display, distinct from the 200-char sanitize ceiling', () => {
    const longError = `network failure ${'x'.repeat(120)} while fetching`;
    const text = renderFirstChip({ board: 'x', count: 0, error: longError })?.textContent ?? '';
    // "label(x)· " prefix + capped detail (<=60 chars + ellipsis).
    const detail = text.split('· ')[1] ?? '';
    expect(detail.length).toBeLessThanOrEqual(61);
    expect(detail.endsWith('…')).toBe(true);
  });

  it('the Tag className allows wrapping instead of forcing single-line overflow', () => {
    const cls = renderFirstChip({ board: 'greenhouse', count: 4, error: 'boom' })?.className ?? '';
    expect(cls).toContain('whitespace-normal');
    expect(cls).toContain('break-words');
    expect(cls).not.toContain('opacity-75');
  });
});
