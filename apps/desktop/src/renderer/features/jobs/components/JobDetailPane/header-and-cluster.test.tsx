/**
 * JobDetailPane — RowMatchScore in the header, cross-board cluster split
 * (ADR-029), and the header action cluster's narrow-pane wrap contract.
 */

import { beforeEach, describe, expect, it } from 'vitest';
import { act, render, screen, within } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import type { Posting } from '@/features/jobs/types';

import {
  formatRelativeTime,
  JobDetailPane,
  makePosting,
  mockNotify,
  mockSplitMutate,
  openPane,
  rerenderPane,
  resetPaneMocks,
} from './harness';

beforeEach(resetPaneMocks);

// Score was removed from list rows; this guards it stays in the detail pane.
describe('JobDetailPane — RowMatchScore renders in detail header', () => {
  it('renders RowMatchScore in the detail pane when a posting is open', async () => {
    await openPane(makePosting('score-visible', { description: 'Full description.' }));
    expect(screen.getByTestId('row-match-score')).toBeInTheDocument();
  });

  it('does NOT render RowMatchScore when posting is null (empty state)', () => {
    render(<JobDetailPane posting={null} formatRelativeTime={formatRelativeTime} />);
    expect(screen.queryByTestId('row-match-score')).not.toBeInTheDocument();
  });
});

describe('JobDetailPane — cross-board cluster split', () => {
  const MEMBERS = [
    { key: 'k1', board: 'linkedin', url: 'https://linkedin.com/job/1' },
    { key: 'k2', board: 'indeed', url: 'https://indeed.com/job/2' },
  ];

  function clustered(members = MEMBERS): Posting {
    return makePosting('clustered', {
      description: 'Full description.',
      clusterId: 'k1',
      clusterCanonical: true,
      clusterMembers: members,
    });
  }

  it('renders the All sources section with one split action (non-canonical member only)', async () => {
    await openPane(clustered());
    expect(screen.getByTestId(TEST_IDS.jobs.clusterMembers)).toBeInTheDocument();
    // k1 is canonical (no split); only k2 offers "Not a duplicate".
    expect(screen.getAllByText('jobs.cluster.notDuplicate')).toHaveLength(1);
  });

  it('split fires markNotDuplicate with the member key + every OTHER member key', async () => {
    await openPane(clustered());

    await act(async () => {
      screen.getByText('jobs.cluster.notDuplicate').click();
    });

    expect(mockSplitMutate).toHaveBeenCalledTimes(1);
    const arg = mockSplitMutate.mock.calls[0]?.[0] as
      { memberKey: string; otherKeys: string[] } | undefined;
    expect(arg?.memberKey).toBe('k2');
    expect(arg?.otherKeys).toEqual(['k1']);
  });

  it('shows the error toast (not the success toast) when the split fails', async () => {
    // React Query calls the caller's onError when the mutation throws; the mock
    // stands in for that so the toast wiring is exercised.
    mockSplitMutate.mockImplementation((_req: unknown, opts?: { onError?: () => void }) =>
      opts?.onError?.()
    );
    await openPane(clustered());

    await act(async () => {
      screen.getByText('jobs.cluster.notDuplicate').click();
    });

    expect(mockNotify.error).toHaveBeenCalledWith({ message: 'jobs.cluster.splitFailed' });
    expect(mockNotify.success).not.toHaveBeenCalled();
  });

  it('the success toast still fires when the refetch collapses the cluster to one member mid-split', async () => {
    // Splitting a 2-member cluster invalidates postings; the refetch drops it to
    // 1 member while the mutation is still pending. The mutation observer (which
    // owns the per-call toast) must stay mounted through that.
    let pending: { onSuccess?: () => void } | undefined;
    mockSplitMutate.mockImplementation((_req: unknown, opts?: { onSuccess?: () => void }) => {
      pending = opts;
    });
    const view = await openPane(clustered());

    await act(async () => {
      screen.getByText('jobs.cluster.notDuplicate').click();
    });
    await rerenderPane(view, clustered(MEMBERS.slice(0, 1)));
    expect(screen.queryByTestId(TEST_IDS.jobs.clusterMembers)).not.toBeInTheDocument();

    act(() => {
      pending?.onSuccess?.();
    });

    expect(mockNotify.success).toHaveBeenCalledWith({ message: 'jobs.cluster.splitDone' });
  });

  it('renders the host fallback label (not the raw url) for a member with no board', async () => {
    await openPane(
      makePosting('nb', {
        description: 'x',
        clusterId: 'k1',
        clusterCanonical: true,
        clusterMembers: [
          { key: 'k1', board: 'linkedin', url: 'https://example.com/job/nb' },
          { key: 'kx', url: 'https://www.jobs.example.com/xyz' },
        ],
      })
    );

    const section = screen.getByTestId(TEST_IDS.jobs.clusterMembers);
    // No board → hostOf(url): hostname minus a leading www., matching the chips.
    expect(within(section).getByText('jobs.example.com')).toBeInTheDocument();
    expect(within(section).queryByText('https://www.jobs.example.com/xyz')).not.toBeInTheDocument();
  });

  it('does NOT render the All sources section for an unclustered posting', async () => {
    await openPane(makePosting('solo', { description: 'x' }));
    expect(screen.queryByTestId(TEST_IDS.jobs.clusterMembers)).not.toBeInTheDocument();
  });
});

// The cluster used to be `shrink-0 flex-wrap`, which pins it at max-content so
// the wrap NEVER fired; at a ~500px detail pane the trailing actions
// (Tailor / Prep / ⋯) overflowed and were clipped by four overflow-hidden
// ancestors. jsdom has no layout engine, so the fix is asserted on the classes
// that produce it.
describe('JobDetailPane — header action cluster wrap contract', () => {
  /** The action cluster is the parent of the Tailor button. */
  async function renderCluster(): Promise<HTMLElement> {
    await openPane(makePosting('wrap', { description: 'x' }));
    const cluster = screen.getByRole('button', { name: /jobs\.tailor/ }).parentElement;
    if (!cluster) throw new Error('action cluster not rendered');
    return cluster;
  }

  it('can shrink below max-content so flex-wrap actually fires', async () => {
    const cluster = await renderCluster();
    expect(cluster.className).toContain('flex-wrap');
    expect(cluster.className).toContain('min-w-0');
    // `shrink-0` is exactly what defeated the wrap.
    expect(cluster.className).not.toContain('shrink-0');
  });

  it('takes its own full-width row under a narrow pane and returns inline when wide', async () => {
    const cluster = await renderCluster();
    // Narrow pane (< 42rem container): full-width row beneath the title.
    expect(cluster.className).toContain('w-full');
    // Wide pane: back inline, right-aligned.
    expect(cluster.className).toContain('@2xl:w-auto');
    expect(cluster.className).toContain('@2xl:justify-end');
  });

  it('marks the header as a container so the decision is pane-width based, not viewport based', async () => {
    const cluster = await renderCluster();
    const header = cluster.parentElement?.parentElement;
    // A container-query variant without a @container ancestor silently no-ops
    // (docs/PATTERNS.md §15) — this is the ancestor that makes @2xl: fire.
    expect(header?.className).toContain('@container');
  });
});
