/**
 * AutopilotCard — held-for-review rows (#1445): badge, old-vs-new block,
 * Apply disabled with a reason, Dismiss. Shared mocks live in ./test-render.
 */

import { describe, expect, it } from 'vitest';
import { fireEvent, screen } from '@testing-library/react';

import {
  dismissState,
  expandCard,
  makeAutopilot,
  makeJob,
  mockDismissReviewMutate,
  mockNotifyError,
  renderCard,
} from './test-render';

const held = {
  ...makeJob('https://example.com/job/held'),
  identityReview: {
    resolvedTitle: 'Plumber',
    resolvedCompany: 'Other Co',
    resolvedUrl: 'https://other.example.com/p/9',
    detectedAt: 1,
  },
};

describe('AutopilotCard — identity review hold', () => {
  it('shows the badge and old-vs-new identities, Apply disabled with a described reason', async () => {
    renderCard(makeAutopilot([held]));
    await expandCard();

    expect(screen.getByText('autopilot.identityReview.badge')).toBeInTheDocument();
    expect(screen.getByText('Software Engineer · Acme')).toBeInTheDocument();
    expect(screen.getByText('Plumber · Other Co')).toBeInTheDocument();
    expect(screen.getByText('https://other.example.com/p/9')).toBeInTheDocument();

    const apply = screen.getByTitle('autopilot.identityReview.applyBlocked');
    expect(apply).toBeDisabled();
    const reasonId = apply.getAttribute('aria-describedby');
    expect(reasonId).toBeTruthy();
    expect(document.getElementById(reasonId as string)).toHaveTextContent(
      'autopilot.identityReview.explain'
    );
  });

  it('Dismiss calls the hook with the autopilot id and url', async () => {
    renderCard(makeAutopilot([held]));
    await expandCard();

    fireEvent.click(screen.getByText('autopilot.identityReview.dismiss'));

    expect(mockDismissReviewMutate).toHaveBeenCalledTimes(1);
    expect(mockDismissReviewMutate.mock.calls[0]?.[0]).toEqual({
      autopilotId: 'ap-1',
      url: 'https://example.com/job/held',
    });
  });

  it('Dismiss failure surfaces the fixed error key', async () => {
    renderCard(makeAutopilot([held]));
    await expandCard();
    fireEvent.click(screen.getByText('autopilot.identityReview.dismiss'));

    const opts = mockDismissReviewMutate.mock.calls[0]?.[1] as { onError: () => void };
    opts.onError();

    expect(mockNotifyError).toHaveBeenCalledWith({
      message: 'autopilot.identityReview.dismissFailed',
    });
  });

  it('disables Dismiss only on the row being dismissed', async () => {
    const other = { ...held, url: 'https://example.com/job/held-2' };
    dismissState.url = held.url;
    renderCard(makeAutopilot([held, other]));
    await expandCard();

    const buttons = screen.getAllByText('autopilot.identityReview.dismiss');
    expect(buttons).toHaveLength(2);
    expect(buttons[0]).toBeDisabled();
    expect(buttons[1]).toBeEnabled();
  });

  it('a normal row has no badge, no review block and an enabled Apply', async () => {
    renderCard(makeAutopilot([makeJob('https://example.com/job/ok')]));
    await expandCard();

    expect(screen.queryByText('autopilot.identityReview.badge')).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.identityReview.dismiss')).not.toBeInTheDocument();
    expect(screen.getByTitle('autopilot.applyJob')).toBeEnabled();
  });
});
