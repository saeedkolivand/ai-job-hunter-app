/**
 * ClusterSourceChips — self-filtering + external-open on click (ADR-029).
 *
 * Strategy:
 *  - useOpenExternal is a spy; @ajh/ui Button/SourceBadge are minimal stubs so a
 *    real <button> carries the chip testid (no AppClient/QueryClient provider).
 *  - i18n is a passthrough returning the key (+ interpolated params).
 *
 * noUncheckedIndexedAccess: array accesses are cast, never `!`-asserted.
 */

import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, p?: Record<string, unknown>) => (p ? `${k}:${Object.values(p).join(',')}` : k),
  }),
}));

vi.mock('@ajh/ui', () => ({
  cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
  Button: ({
    children,
    onClick,
    'aria-label': ariaLabel,
    'data-testid': dataTestId,
  }: {
    children?: React.ReactNode;
    onClick?: () => void;
    'aria-label'?: string;
    'data-testid'?: string;
  }) =>
    React.createElement(
      'button',
      { onClick, 'aria-label': ariaLabel, 'data-testid': dataTestId },
      children
    ),
  SourceBadge: ({ source }: { source: string }) => <span>{source}</span>,
}));

const mockOpen = vi.fn();

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutate: mockOpen }),
}));

import { ClusterSourceChips } from './index';

const MEMBERS = [
  { key: 'k1', board: 'linkedin', url: 'https://linkedin.com/job/1' },
  { key: 'k2', board: 'indeed', url: 'https://indeed.com/job/2' },
  { key: 'k3', board: 'xing', url: 'https://xing.com/job/3' },
];

beforeEach(() => {
  mockOpen.mockClear();
});

describe('ClusterSourceChips', () => {
  it('renders one chip per NON-SELF member (self matched by key)', () => {
    render(
      <ClusterSourceChips members={MEMBERS} selfKey="k1" selfUrl="https://linkedin.com/job/1" />
    );
    // k1 is self → k2 + k3 remain.
    expect(screen.getAllByTestId(TEST_IDS.jobs.clusterSourceChip)).toHaveLength(2);
  });

  it('drops a member matched by url as well as by key', () => {
    render(
      <ClusterSourceChips members={MEMBERS} selfKey="none" selfUrl="https://indeed.com/job/2" />
    );
    // k2 dropped by url; k1 + k3 remain.
    expect(screen.getAllByTestId(TEST_IDS.jobs.clusterSourceChip)).toHaveLength(2);
  });

  it('clicking a chip opens that member url via the external-open hook', async () => {
    const user = userEvent.setup();
    render(
      <ClusterSourceChips members={MEMBERS} selfKey="k1" selfUrl="https://linkedin.com/job/1" />
    );
    const chips = screen.getAllByTestId(TEST_IDS.jobs.clusterSourceChip);
    await user.click(chips[0] as HTMLElement);
    expect(mockOpen).toHaveBeenCalledTimes(1);
    // First non-self member is k2 (indeed).
    expect(mockOpen).toHaveBeenCalledWith('https://indeed.com/job/2');
  });

  it('renders nothing when the cluster has no other member', () => {
    const { container } = render(
      <ClusterSourceChips
        members={[{ key: 'k1', board: 'linkedin', url: 'https://linkedin.com/job/1' }]}
        selfKey="k1"
        selfUrl="https://linkedin.com/job/1"
      />
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing when members is undefined', () => {
    const { container } = render(<ClusterSourceChips selfUrl="https://x.com/1" />);
    expect(container).toBeEmptyDOMElement();
  });
});
