/**
 * AutopilotCard — found-job score: band variant, provisional marker, metric label
 *
 * Shared mocks + fixtures live in ./test-render.
 */

import { describe, expect, it } from 'vitest';
import { screen } from '@testing-library/react';

import type { AutopilotFoundJob } from '@ajh/shared';

import { expandCard, makeAutopilot, makeJob, renderCard } from './test-render';

// ─────────────────────────────────────────────────────────────────────────────
// Found-jobs render the coverage MatchBand
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — found-jobs MatchBand variant', () => {
  it('renders MatchBand with variant=coverage when job.score is present', async () => {
    const job = makeJob('https://example.com/job/scored', 72);
    renderCard(makeAutopilot([job]));

    await expandCard();

    const band = screen.getByTestId('match-band');
    expect(band).toHaveAttribute('data-variant', 'coverage');
    expect(band).toHaveAttribute('data-value', '72');
  });

  it('does NOT render MatchBand when job.score is absent', async () => {
    const job = makeJob('https://example.com/job/no-score'); // no score property
    renderCard(makeAutopilot([job]));

    await expandCard();

    expect(screen.queryByTestId('match-band')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Provisional score marker (PR H, audit root cause 6) — a snippet-based score
// is muted + tilde-prefixed + carries a hover hint; an exact score is plain.
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — provisional score marker', () => {
  it('renders a muted band + "~" prefix + hover title + sr-only text when scoreProvisional is true (HIGH-tier score)', async () => {
    // 82 under variant='coverage' (>=55 threshold) is a HIGH-tier score — the
    // exact case CodeRabbit flagged: MatchBand's `subtle` prop deliberately
    // keeps High bright, so the provisional marker must use `muted` (mutes
    // ALL tiers) instead, or a provisional HIGH would misleadingly stay
    // full-color. The mock recomputes muting from the REAL scoreTier, so this
    // assertion only passes if AutopilotCard passes `muted`, not `subtle`.
    const job = { ...makeJob('https://example.com/job/prov', 82), scoreProvisional: true };
    renderCard(makeAutopilot([job]));

    await expandCard();

    // The native hover hint (title) carries BOTH facts: what the tier claims,
    // and that the number behind it is only an estimate. They answer different
    // questions, so neither may be dropped.
    // The band's own nearest titled ancestor — not just any title on the card.
    const marker = screen.getByTestId('match-band').closest('[title]') as HTMLElement;
    expect(marker.title).toContain('jobs.matchBand.desc.coverage.High');
    expect(marker.title).toContain('autopilot.provisionalScoreHint');
    // Exactly ONE title on this marker — the band must not render its own
    // inside this wrapper, or the inner one wins on hover over the badge and
    // hides the provisional caveat entirely.
    expect(marker.querySelectorAll('[title]')).toHaveLength(0);
    // ...the "~" estimate prefix is visible...
    expect(screen.getByText('~')).toBeInTheDocument();
    // ...an always-present sr-only span carries the same words for screen
    // readers (a `title` alone isn't reliably announced — TrustBadge
    // precedent), and only ONE of them, not one per nested describer...
    const srOnly = marker.querySelectorAll('.sr-only');
    expect(srOnly).toHaveLength(1);
    expect(srOnly[0]?.textContent).toBe(`: ${marker.title}`);
    // The band itself must stay silent here — this wrapper speaks for it.
    expect(screen.getByTestId('match-band')).toHaveAttribute('data-describe', 'false');
    // ...the band IS the High tier (proving this is genuinely a HIGH-score case)...
    const band = screen.getByTestId('match-band');
    expect(band).toHaveAttribute('data-tier', 'High');
    // ...and still renders muted, unlike `subtle`'s High-stays-bright contract.
    expect(band).toHaveAttribute('data-muted', 'true');
  });

  it('renders a plain (non-muted) HIGH band with no marker when scoreProvisional is false', async () => {
    const job = { ...makeJob('https://example.com/job/exact', 82), scoreProvisional: false };
    renderCard(makeAutopilot([job]));

    await expandCard();

    expect(screen.queryByTitle('autopilot.provisionalScoreHint')).not.toBeInTheDocument();
    expect(screen.queryByText('~')).not.toBeInTheDocument();
    expect(screen.queryByText(': autopilot.provisionalScoreHint')).not.toBeInTheDocument();
    const band = screen.getByTestId('match-band');
    expect(band).toHaveAttribute('data-tier', 'High');
    expect(band).toHaveAttribute('data-muted', 'false');
    // The band opts out of describing itself because its WRAPPER now owns the
    // richer copy (metric name + tier description) for every score, provisional
    // or not. The invariant that matters is unchanged: the badge is never a
    // bare, unexplained word — so assert the explanation is actually there,
    // exactly once, rather than which component happens to render it.
    expect(band).toHaveAttribute('data-describe', 'false');
    const marker = band.closest('[title]') as HTMLElement;
    expect(marker.title).toContain('jobs.matchBand.desc.coverage.High');
    expect(marker.querySelectorAll('[title]')).toHaveLength(0);
    expect(marker.querySelectorAll('.sr-only')).toHaveLength(1);
  });

  it('treats an absent scoreProvisional field (older records) as non-provisional', async () => {
    // makeJob() sets no scoreProvisional — the legacy record shape.
    const job = makeJob('https://example.com/job/legacy', 82);
    renderCard(makeAutopilot([job]));

    await expandCard();

    expect(screen.queryByTitle('autopilot.provisionalScoreHint')).not.toBeInTheDocument();
    expect(screen.getByTestId('match-band')).toHaveAttribute('data-muted', 'false');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// scoreSource — the metric label flips to "Match %" ONLY for a job the backend
// actually re-ranked through the semantic kernel (ADR-020 addendum).
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — score metric label', () => {
  /** Expand the found-jobs panel and return the band + its titled wrapper. */
  async function renderScored(job: AutopilotFoundJob) {
    renderCard(makeAutopilot([job]));
    await expandCard();
    const band = screen.getByTestId('match-band');
    return { band, marker: band.closest('[title]') as HTMLElement };
  }

  it('labels a keyword-scored job "Keyword Coverage %" on the coverage scale', async () => {
    const job = { ...makeJob('https://example.com/job/kw', 60), scoreSource: 'keyword' as const };
    const { band, marker } = await renderScored(job);

    expect(marker.title).toContain('autopilot.scoreLabel.coverage');
    expect(marker.title).not.toContain('autopilot.scoreLabel.combined');
    expect(band).toHaveAttribute('data-variant', 'coverage');
    // 60 is High on the coverage scale (>=55) but only Medium on the combined
    // one (>=50) — so the tier here also proves the right cut points ran, not
    // just that the right word was printed.
    expect(band).toHaveAttribute('data-tier', 'High');
    expect(marker.querySelectorAll('.sr-only')[0]?.textContent).toBe(`: ${marker.title}`);
  });

  it('flips to "Match %" on the combined scale when the backend re-ranked the job', async () => {
    const job = { ...makeJob('https://example.com/job/sem', 60), scoreSource: 'combined' as const };
    const { band, marker } = await renderScored(job);

    expect(marker.title).toContain('autopilot.scoreLabel.combined');
    expect(marker.title).not.toContain('autopilot.scoreLabel.coverage');
    expect(band).toHaveAttribute('data-variant', 'combined');
    // Same 60, different metric → Medium, not High. A label-only flip that left
    // the variant on 'coverage' would still read High here and fail.
    expect(band).toHaveAttribute('data-tier', 'Medium');
    expect(marker.title).toContain('jobs.matchBand.desc.combined.Medium');
  });

  it('treats an absent scoreSource (every pre-existing record) as keyword coverage', async () => {
    // makeJob() sets no scoreSource — the legacy record shape, and also what a
    // run with semantic scoring OFF writes.
    const { band, marker } = await renderScored(makeJob('https://example.com/job/legacy', 60));

    expect(marker.title).toContain('autopilot.scoreLabel.coverage');
    expect(band).toHaveAttribute('data-variant', 'coverage');
  });

  // ── the mixed-scale affordance ──────────────────────────────────────────
  //
  // After a semantic re-rank the list holds TWO scales and is sorted in two
  // blocks, so a combined 58 legitimately sits above a keyword 62. The metric
  // was only ever in the tier colour and the sr-only text, which reads to a
  // sighted user as a sorting bug.

  /** Expand the found-jobs panel for a whole list. */
  async function renderList(jobs: AutopilotFoundJob[]) {
    renderCard(makeAutopilot(jobs));
    await expandCard();
  }

  const combined = (url: string, score: number): AutopilotFoundJob => ({
    ...makeJob(url, score),
    scoreSource: 'combined' as const,
  });
  const keyword = (url: string, score: number): AutopilotFoundJob => ({
    ...makeJob(url, score),
    scoreSource: 'keyword' as const,
  });

  it('names each row’s metric when the list mixes the two scales', async () => {
    // The exact reported shape: the re-ranked head scores LOWER than the
    // keyword tail, so without a visible metric the order looks broken.
    await renderList([combined('https://example.com/a', 58), keyword('https://example.com/b', 62)]);

    expect(screen.getByText('autopilot.scoreAbbr.combined')).toBeInTheDocument();
    expect(screen.getByText('autopilot.scoreAbbr.coverage')).toBeInTheDocument();
  });

  it('adds nothing when every score is on the same scale', async () => {
    // The overwhelmingly common case (semantic scoring off, or a run where
    // every job re-ranked): an identical label on every row is pure noise.
    await renderList([keyword('https://example.com/a', 62), keyword('https://example.com/b', 40)]);

    expect(screen.queryByText('autopilot.scoreAbbr.coverage')).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.scoreAbbr.combined')).not.toBeInTheDocument();
  });

  it('ignores unscored rows when deciding whether the list mixes', async () => {
    // An unscored job renders no band at all, so it cannot be one of the two
    // scales — counting it would label a uniform list.
    await renderList([keyword('https://example.com/a', 62), makeJob('https://example.com/b')]);

    expect(screen.queryByText('autopilot.scoreAbbr.coverage')).not.toBeInTheDocument();
  });

  it('keeps the metric out of the accessible name, which already carries it', async () => {
    // aria-hidden: the sr-only span next to the band announces the FULL label
    // ("Keyword Coverage %"), so an announced abbreviation would be a second,
    // shorter duplicate of the same fact.
    await renderList([combined('https://example.com/a', 58), keyword('https://example.com/b', 62)]);

    expect(screen.getByText('autopilot.scoreAbbr.coverage')).toHaveAttribute('aria-hidden', 'true');
  });

  it('keeps the provisional caveat alongside the flipped label', async () => {
    // A re-ranked aggregator job is BOTH semantic and snippet-derived: the
    // label flips, and the "~"/muted/caveat treatment must survive.
    const job = {
      ...makeJob('https://example.com/job/both', 60),
      scoreSource: 'combined' as const,
      scoreProvisional: true,
    };
    const { band, marker } = await renderScored(job);

    expect(marker.title).toContain('autopilot.scoreLabel.combined');
    expect(marker.title).toContain('autopilot.provisionalScoreHint');
    expect(screen.getByText('~')).toBeInTheDocument();
    expect(band).toHaveAttribute('data-muted', 'true');
  });
});
