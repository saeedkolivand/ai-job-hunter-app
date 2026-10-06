/**
 * AutopilotCard — found-job row: open + viewed, AI note, clustering, posted date
 *
 * Shared mocks + fixtures live in ./test-render.
 */

import { describe, expect, it } from 'vitest';
import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { AutopilotFoundJob } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

import {
  cardHeader,
  expandCard,
  makeAutopilot,
  makeJob,
  mockOpenExternal,
  mockPersistJobAsync,
  mockSplitMutate,
  renderCard,
  state,
} from './test-render';

// ─────────────────────────────────────────────────────────────────────────────
// handleJobClick — openExternal + persistJob + viewed badge
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — handleJobClick', () => {
  it('calls openExternal.mutate with the job url', async () => {
    const user = userEvent.setup();
    const job = makeJob('https://example.com/job/42');
    renderCard(makeAutopilot([job]));

    // Expand the header to show the found-jobs panel.
    await user.click(cardHeader());

    const jobButton = screen.getByTitle('autopilot.viewJob');
    await user.click(jobButton);

    expect(mockOpenExternal).toHaveBeenCalledWith('https://example.com/job/42');
  });

  it('calls persistJob.mutateAsync with interactionType: viewed and the job url', async () => {
    const user = userEvent.setup();
    const job = makeJob('https://example.com/job/42');
    renderCard(makeAutopilot([job]));

    await user.click(cardHeader());

    const jobButton = screen.getByTitle('autopilot.viewJob');
    await act(async () => {
      await user.click(jobButton);
    });

    expect(mockPersistJobAsync).toHaveBeenCalledTimes(1);
    const callArg = mockPersistJobAsync.mock.calls[0]?.[0] as Record<string, unknown> | undefined;
    expect(callArg?.interactionType).toBe('viewed');
    expect((callArg?.job as Record<string, unknown> | undefined)?.url).toBe(
      'https://example.com/job/42'
    );
  });

  it('shows the Eye/viewed badge for a url that is in viewedUrls', async () => {
    const jobUrl = 'https://example.com/job/viewed';
    state.viewed = [{ url: jobUrl }];

    renderCard(makeAutopilot([makeJob(jobUrl)]));

    await expandCard();

    // The viewed badge (t('jobs.viewed') → 'jobs.viewed') should appear.
    expect(screen.getByText('jobs.viewed')).toBeInTheDocument();
  });

  it('shows the viewed badge for a url from openedData (opened counts as viewed)', async () => {
    const jobUrl = 'https://example.com/job/opened';
    state.opened = [{ url: jobUrl }];

    renderCard(makeAutopilot([makeJob(jobUrl)]));

    await expandCard();

    expect(screen.getByText('jobs.viewed')).toBeInTheDocument();
  });

  it('does NOT show the viewed badge for an unvisited url', async () => {
    state.viewed = [];
    state.opened = [];

    renderCard(makeAutopilot([makeJob('https://example.com/job/unseen')]));

    await expandCard();

    expect(screen.queryByText('jobs.viewed')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleJobClick — persistJob `id` (regression: InteractionStore::upsert keys
// on (job_id, interaction_type); job_id defaults to "" server-side, so every
// autopilot found job without an id collided on the SAME slot and only the
// most-recently-opened job ever showed the Viewed badge — .claude/scratch/
// best-matches.md's "known trap".)
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — handleJobClick persistJob id (viewed-badge collision regression)', () => {
  it('two distinct found jobs produce two distinct persistJob payload ids', async () => {
    const user = userEvent.setup();
    const jobA = makeJob('https://example.com/job/a');
    const jobB = makeJob('https://example.com/job/b');
    renderCard(makeAutopilot([jobA, jobB]));

    await user.click(cardHeader());

    const [buttonA, buttonB] = screen.getAllByTitle('autopilot.viewJob');
    if (!buttonA || !buttonB) throw new Error('expected two job view buttons');

    await act(async () => {
      await user.click(buttonA);
    });
    await act(async () => {
      await user.click(buttonB);
    });

    expect(mockPersistJobAsync).toHaveBeenCalledTimes(2);
    const callA = mockPersistJobAsync.mock.calls[0]?.[0] as
      { job: Record<string, unknown> } | undefined;
    const callB = mockPersistJobAsync.mock.calls[1]?.[0] as
      { job: Record<string, unknown> } | undefined;
    expect(callA?.job.id).toBe('https://example.com/job/a');
    expect(callB?.job.id).toBe('https://example.com/job/b');
    expect(callA?.job.id).not.toBe(callB?.job.id);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// handleJobClick — persistJob rejection (swallowed catch; #3)
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — handleJobClick persistJob rejection', () => {
  it('openExternal.mutate still fires when persistJob.mutateAsync rejects', async () => {
    mockPersistJobAsync.mockRejectedValueOnce(new Error('network'));

    const user = userEvent.setup();
    const job = makeJob('https://example.com/job/persist-fail');
    renderCard(makeAutopilot([job]));

    await user.click(cardHeader());

    await act(async () => {
      await user.click(screen.getByTitle('autopilot.viewJob'));
    });

    // openExternal fires before the try/catch around persistJob.
    expect(mockOpenExternal).toHaveBeenCalledWith('https://example.com/job/persist-fail');
    // No unhandled rejection — test runner would fail if one escaped.
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// assistantNotes — Phase 4 AI note (read-only, plain text)
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — assistantNotes', () => {
  it('renders the AI note when job.assistantNotes is present', async () => {
    const job = {
      ...makeJob('https://example.com/job/noted'),
      assistantNotes: 'Great fit — highlight your Rust experience.',
    };
    renderCard(makeAutopilot([job]));

    await expandCard();

    expect(screen.getByText('Great fit — highlight your Rust experience.')).toBeInTheDocument();
    expect(screen.getByRole('note', { name: 'autopilot.aiNote' })).toBeInTheDocument();
  });

  it('does NOT render an AI note block when job.assistantNotes is absent', async () => {
    const job = makeJob('https://example.com/job/no-note');
    renderCard(makeAutopilot([job]));

    await expandCard();

    expect(screen.queryByRole('note')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Cross-board clustering (ADR-029) — one rendered row per cluster
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — cross-board clustering', () => {
  it('renders one row per cluster — the non-canonical member is hidden', async () => {
    const user = userEvent.setup();
    const jobs: AutopilotFoundJob[] = [
      {
        title: 'Canonical role',
        company: 'Acme',
        url: 'https://a.com/1',
        foundAt: 0,
        clusterCanonical: true,
      },
      {
        title: 'Hidden duplicate',
        company: 'Acme',
        url: 'https://b.com/2',
        foundAt: 0,
        clusterCanonical: false,
      },
    ];
    renderCard(makeAutopilot(jobs));

    await user.click(cardHeader());

    // Only the canonical member is listed; the found-count reflects clusters (1).
    expect(screen.getByText('Canonical role')).toBeInTheDocument();
    expect(screen.queryByText('Hidden duplicate')).not.toBeInTheDocument();
    expect(screen.getByText('autopilot.foundJobs · 1')).toBeInTheDocument();
  });

  it('always shows unclustered (legacy) rows — no cluster annotation', async () => {
    const user = userEvent.setup();
    renderCard(
      makeAutopilot([{ title: 'Legacy role', company: 'Acme', url: 'https://a.com/1', foundAt: 0 }])
    );

    await user.click(cardHeader());

    expect(screen.getByText('Legacy role')).toBeInTheDocument();
    expect(screen.getByText('autopilot.foundJobs · 1')).toBeInTheDocument();
  });

  it('split action fires markNotDuplicate with memberKey, otherKeys AND autopilotId', async () => {
    const user = userEvent.setup();
    const job: AutopilotFoundJob = {
      title: 'Clustered role',
      company: 'Acme',
      url: 'https://a.com/1',
      foundAt: 0,
      clusterCanonical: true,
      clusterId: 'k1',
      clusterMembers: [
        { key: 'k1', board: 'linkedin', url: 'https://a.com/1' },
        { key: 'k2', board: 'indeed', url: 'https://b.com/2' },
      ],
    };
    // makeAutopilot fixes _id: 'ap-1' — the autopilotId the split must carry.
    renderCard(makeAutopilot([job]));

    // Expand the found-jobs panel so the cluster sub-row (with the split) mounts.
    await user.click(cardHeader());

    await user.click(screen.getByTestId(TEST_IDS.jobs.clusterSplitButton));

    expect(mockSplitMutate).toHaveBeenCalledTimes(1);
    const arg = mockSplitMutate.mock.calls[0]?.[0] as
      { memberKey: string; otherKeys: string[]; autopilotId?: string } | undefined;
    // memberKey = canonical key; otherKeys = the rest; autopilotId scopes the
    // per-record recompute (ADR-029 §h — only this call site sends it).
    expect(arg).toEqual({ memberKey: 'k1', otherKeys: ['k2'], autopilotId: 'ap-1' });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// postedAt date chip — display precedent: same helper+namespace the Jobs page
// uses (PostingListItem/index.tsx:121-122); absolute-time title tooltip mirrors
// ApplicationRow:231. Several boards ship no publish date, so absence must
// render nothing, not "NaN ago".
// ─────────────────────────────────────────────────────────────────────────────

describe('AutopilotCard — postedAt date chip', () => {
  it('renders the relative time + an absolute-time title tooltip for a dated job', async () => {
    const postedAt = Date.now() - 5 * 60_000; // 5 minutes ago
    const job = { ...makeJob('https://example.com/job/dated'), postedAt };
    renderCard(makeAutopilot([job]));

    await expandCard();

    // The identity t() mock echoes the resolved i18n key back verbatim.
    const chip = screen.getByText(/jobs\.timeMinutesAgo/);
    expect(chip).toHaveAttribute('title', new Date(postedAt).toLocaleString());
  });

  it('renders no chip at all when postedAt is absent (board ships no publish date)', async () => {
    const job = makeJob('https://example.com/job/undated'); // no postedAt
    renderCard(makeAutopilot([job]));

    await expandCard();

    expect(screen.queryByText(/jobs\.time/)).not.toBeInTheDocument();
  });

  // Regression (CodeRabbit round 1): `job.postedAt && (...)` is the classic
  // 0-&&-JSX footgun — a `postedAt: 0` (epoch) job would render a bare stray
  // "0" text node instead of the chip, AND it disagreed with
  // `sortFoundJobsByDate`, which already treats 0 as dated via
  // `typeof === 'number'`. One presence contract now covers both the render
  // guard and the sort banding.
  it('treats postedAt: 0 as dated — chip renders (no stray "0" text), and the row sorts in the dated band', async () => {
    const user = userEvent.setup();
    const epochJob = { ...makeJob('https://example.com/job/epoch'), postedAt: 0 };
    const undatedJob = makeJob('https://example.com/job/no-date');
    renderCard(makeAutopilot([undatedJob, epochJob]));

    await user.click(cardHeader());

    // Display half of the contract: the JSX branch renders (a real titled
    // <span>), not the OLD `job.postedAt && (...)` guard's failure mode —
    // `0 && (<span>…</span>)` short-circuits to the bare number `0`, which
    // React renders as a stray "0" text node instead of the chip. The
    // relative-time TEXT itself is a separate, pre-existing concern
    // (`useFormatRelativeTime`'s own `if (!timestamp)` falsy-check also
    // treats 0 as absent — out of scope here; `title` is computed straight
    // from `job.postedAt`, not through that hook, so it's still a precise
    // signal that this is the real chip, not the footgun's stray digit).
    const epochRow = document.querySelector('[data-job-url="https://example.com/job/epoch"]');
    const chip = epochRow?.querySelector('span[title]');
    expect(chip).not.toBeNull();
    expect(chip).toHaveAttribute('title', new Date(0).toLocaleString());
    // The footgun's tell: no bare "0" text node anywhere in the row.
    expect(
      within(epochRow as HTMLElement).queryByText('0', { exact: true })
    ).not.toBeInTheDocument();

    // Sort half of the contract: the epoch job must band as DATED (ahead of
    // the undated one), not fall through to the undated/trailing band.
    await user.click(screen.getByRole('button', { name: 'jobs.sortNewest' }));
    const order = Array.from(document.querySelectorAll('[data-job-url]')).map((el) =>
      el.getAttribute('data-job-url')
    );
    expect(order).toEqual(['https://example.com/job/epoch', 'https://example.com/job/no-date']);
  });
});
