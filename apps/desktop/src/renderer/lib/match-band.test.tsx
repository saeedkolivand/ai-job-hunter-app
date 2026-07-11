/**
 * match-band — scoreTier boundary tests for both variants + MatchBand subtle prop.
 *
 * Tests the pure scoreTier function (no rendering needed) to pin tier
 * boundaries deterministically:
 *   variant='combined' (default): ≥75 High, ≥50 Medium, <50 Low
 *   variant='coverage':           ≥55 High, ≥30 Medium, <30 Low
 *
 * Also renders MatchBand with subtle=true/false to pin the muted-neutral
 * styling: High always stays bright; Medium/Low go muted when subtle. And
 * with muted=true, which — unlike subtle — mutes EVERY tier including High.
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

// Minimal stubs for MatchBand render tests
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
vi.mock('@ajh/ui', () => ({
  Tag: ({
    children,
    className,
  }: {
    children: React.ReactNode;
    color?: string;
    className?: string;
  }) => <span className={className ?? ''}>{children}</span>,
  cn: (...args: (string | undefined | false | null)[]) => args.filter(Boolean).join(' '),
}));

import { MatchBand, scoreTier } from './match-band';

// ─────────────────────────────────────────────────────────────────────────────
// variant='combined' (default) — boundaries at 75 and 50
// ─────────────────────────────────────────────────────────────────────────────

describe("scoreTier — variant='combined' (default)", () => {
  it('returns High for value exactly at 75', () => {
    expect(scoreTier(75).key).toBe('High');
  });

  it('returns High for value above 75', () => {
    expect(scoreTier(100).key).toBe('High');
    expect(scoreTier(90).key).toBe('High');
    expect(scoreTier(76).key).toBe('High');
  });

  it('returns Medium for value exactly at 50', () => {
    expect(scoreTier(50).key).toBe('Medium');
  });

  it('returns Medium for value in [50, 74]', () => {
    expect(scoreTier(74).key).toBe('Medium');
    expect(scoreTier(60).key).toBe('Medium');
    expect(scoreTier(51).key).toBe('Medium');
  });

  it('returns Low for value exactly at 49 (boundary below Medium)', () => {
    expect(scoreTier(49).key).toBe('Low');
  });

  it('returns Low for value below 50', () => {
    expect(scoreTier(0).key).toBe('Low');
    expect(scoreTier(1).key).toBe('Low');
    expect(scoreTier(30).key).toBe('Low');
  });

  it('omitting variant is identical to combined (regression guard)', () => {
    // Omitting variant must produce the same result as passing 'combined' explicitly.
    expect(scoreTier(75)).toEqual(scoreTier(75, 'combined'));
    expect(scoreTier(50)).toEqual(scoreTier(50, 'combined'));
    expect(scoreTier(49)).toEqual(scoreTier(49, 'combined'));
  });

  it('returns the correct Tag color for each tier', () => {
    expect(scoreTier(75).color).toBe('success');
    expect(scoreTier(50).color).toBe('warning');
    expect(scoreTier(49).color).toBe('error');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// variant='coverage' — boundaries at 55 and 30
// ─────────────────────────────────────────────────────────────────────────────

describe("scoreTier — variant='coverage'", () => {
  it('returns High for value exactly at 55', () => {
    expect(scoreTier(55, 'coverage').key).toBe('High');
  });

  it('returns High for value above 55', () => {
    expect(scoreTier(100, 'coverage').key).toBe('High');
    expect(scoreTier(80, 'coverage').key).toBe('High');
    expect(scoreTier(56, 'coverage').key).toBe('High');
  });

  it('returns Medium for value exactly at 54 (boundary below High)', () => {
    expect(scoreTier(54, 'coverage').key).toBe('Medium');
  });

  it('returns Medium for value exactly at 30', () => {
    expect(scoreTier(30, 'coverage').key).toBe('Medium');
  });

  it('returns Medium for value in [30, 54]', () => {
    expect(scoreTier(54, 'coverage').key).toBe('Medium');
    expect(scoreTier(40, 'coverage').key).toBe('Medium');
    expect(scoreTier(31, 'coverage').key).toBe('Medium');
  });

  it('returns Low for value exactly at 29 (boundary below Medium)', () => {
    expect(scoreTier(29, 'coverage').key).toBe('Low');
  });

  it('returns Low for value below 30', () => {
    expect(scoreTier(0, 'coverage').key).toBe('Low');
    expect(scoreTier(10, 'coverage').key).toBe('Low');
    expect(scoreTier(28, 'coverage').key).toBe('Low');
  });

  it('returns the correct Tag color for each coverage tier', () => {
    expect(scoreTier(55, 'coverage').color).toBe('success');
    expect(scoreTier(30, 'coverage').color).toBe('warning');
    expect(scoreTier(29, 'coverage').color).toBe('error');
  });

  it('coverage 54 is Medium while combined 54 is also Medium — variants converge mid-range', () => {
    // Both variants map 54 → Medium, but via different ceiling rules.
    expect(scoreTier(54, 'coverage').key).toBe('Medium');
    expect(scoreTier(54, 'combined').key).toBe('Medium');
  });

  it('coverage 55 is High but combined 55 is still Medium (diverges at the new boundary)', () => {
    expect(scoreTier(55, 'coverage').key).toBe('High');
    expect(scoreTier(55, 'combined').key).toBe('Medium');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// MatchBand — subtle prop rendering
// ─────────────────────────────────────────────────────────────────────────────

describe('MatchBand — subtle=false (default)', () => {
  it('renders i18n key label for all tiers without muted classes', () => {
    const { rerender } = render(<MatchBand value={80} />);
    expect(screen.getByText('jobs.matchBand.High').className).not.toContain('text-foreground/70');

    rerender(<MatchBand value={60} />);
    expect(screen.getByText('jobs.matchBand.Medium').className).not.toContain('text-foreground/70');

    rerender(<MatchBand value={20} />);
    expect(screen.getByText('jobs.matchBand.Low').className).not.toContain('text-foreground/70');
  });
});

describe('MatchBand — subtle=true', () => {
  it('High stays bright — no muted classes', () => {
    render(<MatchBand value={80} subtle />);
    const el = screen.getByText('jobs.matchBand.High');
    expect(el.className).not.toContain('text-foreground/70');
    expect(el.className).not.toContain('bg-muted');
  });

  it('Medium gets muted-neutral styling', () => {
    render(<MatchBand value={60} subtle />);
    const el = screen.getByText('jobs.matchBand.Medium');
    expect(el.className).toContain('text-foreground/70');
    expect(el.className).toContain('bg-muted');
  });

  it('Low gets muted-neutral styling', () => {
    render(<MatchBand value={20} subtle />);
    const el = screen.getByText('jobs.matchBand.Low');
    expect(el.className).toContain('text-foreground/70');
    expect(el.className).toContain('bg-muted');
  });

  it('boundary: value=75 is High → stays bright even with subtle', () => {
    render(<MatchBand value={75} subtle />);
    const el = screen.getByText('jobs.matchBand.High');
    expect(el.className).not.toContain('bg-muted');
  });
});

describe('MatchBand — muted=true (mutes ALL tiers, unlike subtle)', () => {
  it('High gets muted-neutral styling (distinct from subtle, which keeps High bright)', () => {
    render(<MatchBand value={80} muted />);
    const el = screen.getByText('jobs.matchBand.High');
    expect(el.className).toContain('text-foreground/70');
    expect(el.className).toContain('bg-muted');
  });

  it('Medium gets muted-neutral styling', () => {
    render(<MatchBand value={60} muted />);
    const el = screen.getByText('jobs.matchBand.Medium');
    expect(el.className).toContain('text-foreground/70');
    expect(el.className).toContain('bg-muted');
  });

  it('Low gets muted-neutral styling', () => {
    render(<MatchBand value={20} muted />);
    const el = screen.getByText('jobs.matchBand.Low');
    expect(el.className).toContain('text-foreground/70');
    expect(el.className).toContain('bg-muted');
  });

  it('muted and subtle together still mute High (muted wins)', () => {
    render(<MatchBand value={80} muted subtle />);
    const el = screen.getByText('jobs.matchBand.High');
    expect(el.className).toContain('bg-muted');
  });
});
