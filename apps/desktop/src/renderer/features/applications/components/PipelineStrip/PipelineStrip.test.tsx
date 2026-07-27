/**
 * PipelineStrip — six always-present stat cards, toggle-filter semantics and the
 * client-derived overdue badge.
 *
 * `@ajh/ui` is imported real (Button/Tag are exercised); only `@ajh/translations`
 * is stubbed so keys assert directly.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { Application } from '@ajh/shared';

import { PipelineStrip } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const NOW = 1_700_000_000_000;

function makeApp(overrides: Partial<Application>): Application {
  return {
    id: 'app-1',
    status: 'applied',
    createdAt: NOW,
    updatedAt: NOW,
    jobUrl: '',
    board: '',
    company: 'Acme',
    title: 'Engineer',
    candidate: 'Jane',
    answers: [],
    brief: '',
    notes: '',
    comp: '',
    jobDescription: '',
    jobSummary: '',
    contactName: '',
    contactEmail: '',
    ...overrides,
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
});
afterEach(() => vi.useRealTimers());

const cards = () => screen.getAllByRole('button');

describe('PipelineStrip', () => {
  it('always renders all six cards, zeros included', () => {
    render(<PipelineStrip applications={[]} active={null} onSelect={vi.fn()} />);

    expect(cards()).toHaveLength(6);
    for (const id of ['saved', 'applied', 'screening', 'interviewing', 'offer', 'closed']) {
      expect(screen.getByText(`applications.pipeline.${id}`)).toBeInTheDocument();
    }
    // Six zero counts.
    expect(screen.getAllByText('0')).toHaveLength(6);
  });

  it('aggregates the four end-states into the Closed count', () => {
    render(
      <PipelineStrip
        applications={[
          makeApp({ id: 'a', status: 'accepted' }),
          makeApp({ id: 'b', status: 'rejected' }),
          makeApp({ id: 'c', status: 'ghosted' }),
          makeApp({ id: 'd', status: 'withdrawn' }),
          makeApp({ id: 'e', status: 'saved' }),
        ]}
        active={null}
        onSelect={vi.fn()}
      />
    );

    const closed = screen.getByText('applications.pipeline.closed').closest('button');
    expect(closed).toHaveTextContent('4');
    const saved = screen.getByText('applications.pipeline.saved').closest('button');
    expect(saved).toHaveTextContent('1');
  });

  it('clicking an inactive card selects that group', () => {
    const onSelect = vi.fn();
    render(<PipelineStrip applications={[]} active={null} onSelect={onSelect} />);

    fireEvent.click(screen.getByText('applications.pipeline.offer'));
    expect(onSelect).toHaveBeenCalledWith('offer');
  });

  it('clicking the ACTIVE card clears the filter (null)', () => {
    const onSelect = vi.fn();
    render(<PipelineStrip applications={[]} active="offer" onSelect={onSelect} />);

    fireEvent.click(screen.getByText('applications.pipeline.offer'));
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it('exposes selection through aria-pressed (one card at a time)', () => {
    render(<PipelineStrip applications={[]} active="screening" onSelect={vi.fn()} />);

    const pressed = cards().filter((c) => c.getAttribute('aria-pressed') === 'true');
    expect(pressed).toHaveLength(1);
    expect(pressed[0]).toHaveTextContent('applications.pipeline.screening');
  });

  it('shows the overdue badge only when a reminder has actually passed', () => {
    const { unmount } = render(
      <PipelineStrip
        applications={[
          makeApp({ id: 'a', nextActionAt: NOW - 1 }),
          makeApp({ id: 'b', nextActionAt: NOW + 86_400_000 }),
        ]}
        active={null}
        onSelect={vi.fn()}
      />
    );
    expect(screen.getByText('applications.pipeline.overdue')).toBeInTheDocument();
    unmount();

    render(
      <PipelineStrip
        applications={[makeApp({ id: 'b', nextActionAt: NOW + 86_400_000 })]}
        active={null}
        onSelect={vi.fn()}
      />
    );
    expect(screen.queryByText('applications.pipeline.overdue')).not.toBeInTheDocument();
  });
});
