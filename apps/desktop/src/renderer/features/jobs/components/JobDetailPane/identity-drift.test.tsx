/**
 * JobDetailPane — identity drift (#1445): the other advert's text is never
 * shown or scored, not even before the persist verdict resolves.
 */

import { beforeEach, describe, expect, it } from 'vitest';
import { act, fireEvent, screen } from '@testing-library/react';

import {
  flushMicrotasks,
  makePosting,
  mockScoreJob,
  mockUpdateDescMutateAsync,
  openPane,
  resetPaneMocks,
  resolveReturns,
} from './harness';

beforeEach(resetPaneMocks);

const OTHER_AD = 'The completely different plumber advert, long enough to win the merge.';

function resolveOtherAd() {
  resolveReturns({
    data: {
      description: OTHER_AD,
      title: 'Plumber',
      company: 'Other Co',
      url: 'https://o.example/1',
    },
    isFetched: true,
  });
}

describe('JobDetailPane — identity drift', () => {
  it('on identityDrift shows the notice, never the resolved text, and does not score', async () => {
    mockUpdateDescMutateAsync.mockResolvedValue('identityDrift');
    resolveOtherAd();
    await openPane(makePosting('d1', { description: '' }));
    await flushMicrotasks(2);

    expect(screen.getByRole('note')).toHaveTextContent(
      'jobs.identityDrift.notice|Plumber|Other Co'
    );
    expect(screen.queryByText(OTHER_AD)).not.toBeInTheDocument();
    expect(mockScoreJob).not.toHaveBeenCalled();
  });

  it('does not render the resolved text while the verdict is still pending', async () => {
    mockUpdateDescMutateAsync.mockReturnValue(new Promise(() => {}));
    resolveOtherAd();
    await openPane(makePosting('d2', { description: 'Stored short ad.' }));

    expect(screen.queryByText(OTHER_AD)).not.toBeInTheDocument();
    expect(screen.getByText('Stored short ad.')).toBeInTheDocument();
    expect(mockScoreJob).not.toHaveBeenCalled();
  });

  it('persist failure with empty stored text shows error + retry, never the resolved text', async () => {
    mockUpdateDescMutateAsync.mockRejectedValue(new Error('rejected'));
    resolveOtherAd();
    await openPane(makePosting('d-fail', { description: '' }));
    await flushMicrotasks(2);

    expect(screen.queryByText(OTHER_AD)).not.toBeInTheDocument();
    expect(screen.getByText('jobs.descriptionLoadError')).toBeInTheDocument();
    expect(screen.getByText('jobs.loadFullDescription')).toBeInTheDocument();
    expect(mockScoreJob).not.toHaveBeenCalled();
  });

  it("'unverified' with stored text: stored shown, resolved hidden, not scored, no notice", async () => {
    mockUpdateDescMutateAsync.mockResolvedValue('unverified');
    resolveOtherAd();
    await openPane(makePosting('d-unv', { source: 'aggregator', description: 'Stored ad.' }));
    await flushMicrotasks(2);

    expect(screen.getByText('Stored ad.')).toBeInTheDocument();
    expect(screen.queryByText(OTHER_AD)).not.toBeInTheDocument();
    expect(screen.queryByRole('note')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.loadFullDescription')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.descriptionUnverified')).not.toBeInTheDocument();
    expect(mockScoreJob).not.toHaveBeenCalled();
    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);
  });

  it("'unverified' with empty stored text shows a non-retry line", async () => {
    mockUpdateDescMutateAsync.mockResolvedValue('unverified');
    resolveOtherAd();
    await openPane(makePosting('d-unv-empty', { description: '' }));
    await flushMicrotasks(2);

    expect(screen.queryByText(OTHER_AD)).not.toBeInTheDocument();
    expect(screen.getByText('jobs.descriptionUnverified')).toBeInTheDocument();
    expect(screen.queryByText('jobs.descriptionLoadError')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.loadFullDescription')).not.toBeInTheDocument();
    expect(screen.queryByRole('note')).not.toBeInTheDocument();
    expect(mockScoreJob).not.toHaveBeenCalled();
  });

  it('retry after a failed persist calls updateDescription again and then reveals', async () => {
    mockUpdateDescMutateAsync
      .mockRejectedValueOnce(new Error('rejected'))
      .mockResolvedValueOnce('updated');
    resolveOtherAd();
    await openPane(makePosting('d-retry', { description: '' }));
    await flushMicrotasks(2);
    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);

    await act(async () => {
      fireEvent.click(screen.getByText('jobs.loadFullDescription'));
    });
    await flushMicrotasks(2);

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(2);
    expect(screen.getByText(OTHER_AD)).toBeInTheDocument();
    expect(mockScoreJob).toHaveBeenCalledTimes(1);
  });

  it('drift with NON-empty stored text keeps the stored text, shows the notice, never scores', async () => {
    mockUpdateDescMutateAsync.mockResolvedValue('identityDrift');
    resolveOtherAd();
    await openPane(
      makePosting('d-drift-stored', { source: 'aggregator', description: 'Stored ad.' })
    );
    await flushMicrotasks(2);

    expect(screen.getByText('Stored ad.')).toBeInTheDocument();
    expect(screen.getByRole('note')).toHaveTextContent(
      'jobs.identityDrift.notice|Plumber|Other Co'
    );
    expect(screen.queryByText(OTHER_AD)).not.toBeInTheDocument();
    expect(mockScoreJob).not.toHaveBeenCalled();
  });

  it('stored text longer than the resolved text: no persist, stored shown and scored', async () => {
    resolveReturns({ data: { description: 'short' }, isFetched: true });
    const stored = 'A long stored description that is longer than the resolved one.';
    const posting = makePosting('d-keep', { source: 'aggregator', description: stored });
    await openPane(posting);
    await flushMicrotasks(2);

    expect(mockUpdateDescMutateAsync).not.toHaveBeenCalled();
    expect(screen.getByText(stored)).toBeInTheDocument();
    expect(mockScoreJob).toHaveBeenCalledWith(posting.id);
  });

  it.each(['updated', 'noMatch'])('on %s reveals the resolved text and scores', async (result) => {
    mockUpdateDescMutateAsync.mockResolvedValue(result);
    resolveOtherAd();
    await openPane(makePosting(`d-${result}`, { description: '' }));
    await flushMicrotasks(2);

    expect(screen.getByText(OTHER_AD)).toBeInTheDocument();
    expect(screen.queryByRole('note')).not.toBeInTheDocument();
    expect(mockScoreJob).toHaveBeenCalledTimes(1);
  });
});
