/**
 * PipelineStrip — six always-present stat cards, toggle-filter semantics, the
 * client-derived overdue control, and the PAINTED selection state.
 *
 * `@ajh/ui` is imported real (Button/cn are exercised); only `@ajh/translations`
 * is stubbed so keys assert directly.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { Application } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

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

const cards = () => screen.getAllByTestId(TEST_IDS.applications.pipelineCard);
const card = (group: string) =>
  cards().find((c) => c.getAttribute('data-group') === group) as HTMLElement;

const renderStrip = (props: Partial<React.ComponentProps<typeof PipelineStrip>> = {}) =>
  render(
    <PipelineStrip
      applications={[]}
      active={null}
      onSelect={vi.fn()}
      onShowOverdue={vi.fn()}
      {...props}
    />
  );

describe('PipelineStrip', () => {
  it('always renders all six cards, zeros included', () => {
    renderStrip();

    expect(cards()).toHaveLength(6);
    // Five groups reuse the canonical stage labels; only the aggregate bucket
    // carries a label of its own.
    for (const id of ['saved', 'applied', 'screening', 'interviewing', 'offer']) {
      expect(card(id)).toHaveTextContent(`applications.stages.${id}`);
    }
    expect(card('closed')).toHaveTextContent('applications.pipeline.closed');
    expect(screen.getAllByText('0')).toHaveLength(6);
  });

  it('aggregates the four end-states into the Finished count', () => {
    renderStrip({
      applications: [
        makeApp({ id: 'a', status: 'accepted' }),
        makeApp({ id: 'b', status: 'rejected' }),
        makeApp({ id: 'c', status: 'ghosted' }),
        makeApp({ id: 'd', status: 'withdrawn' }),
        makeApp({ id: 'e', status: 'saved' }),
      ],
    });

    expect(card('closed')).toHaveTextContent('4');
    expect(card('saved')).toHaveTextContent('1');
  });

  it('clicking an inactive card selects that group', () => {
    const onSelect = vi.fn();
    renderStrip({ onSelect });

    fireEvent.click(card('offer'));
    expect(onSelect).toHaveBeenCalledWith('offer');
  });

  it('clicking the ACTIVE card clears the filter (null)', () => {
    const onSelect = vi.fn();
    renderStrip({ active: 'offer', onSelect });

    fireEvent.click(card('offer'));
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it('exposes selection through aria-pressed (one card at a time)', () => {
    renderStrip({ active: 'screening' });

    const pressed = cards().filter((c) => c.getAttribute('aria-pressed') === 'true');
    expect(pressed).toHaveLength(1);
    expect(pressed[0]).toHaveTextContent('applications.stages.screening');
  });

  // Regression: the selected card previously differed ONLY by a layered
  // `bg-*`/`border-*` utility, which loses to `.surface-card`'s unlayered
  // background/border — it painted pixel-identical to an unselected card.
  it('marks selection with a ring (an uncontested box-shadow), not a bg/border swap', () => {
    renderStrip({ active: 'offer' });

    // Token-exact: the Button base carries `focus-visible:ring-2`, so a substring
    // check would pass on an unselected card too.
    const classes = (el: HTMLElement) => el.className.split(/\s+/);
    const selected = classes(card('offer'));
    const other = classes(card('saved'));

    expect(selected).toContain('ring-2');
    expect(selected).toContain('ring-inset');
    expect(selected).toContain('ring-brand/70');
    // The losing utilities must not come back.
    expect(selected).not.toContain('bg-brand/[0.07]');
    expect(selected).not.toContain('border-brand/45');
    expect(other).not.toContain('ring-2');
  });

  it('carries a non-colour cue on the selected card only', () => {
    const { container } = renderStrip({ active: 'offer' });

    // One check glyph in the whole strip, inside the selected card.
    const glyphs = container.querySelectorAll('svg.lucide-check');
    expect(glyphs).toHaveLength(1);
    expect(card('offer').contains(glyphs[0] ?? null)).toBe(true);
  });

  it('shows the overdue control only when a reminder has actually passed', () => {
    const { unmount } = renderStrip({
      applications: [
        makeApp({ id: 'a', nextActionAt: NOW - 1 }),
        makeApp({ id: 'b', nextActionAt: NOW + 86_400_000 }),
      ],
    });
    expect(screen.getByText('applications.pipeline.overdue')).toBeInTheDocument();
    unmount();

    renderStrip({ applications: [makeApp({ id: 'b', nextActionAt: NOW + 86_400_000 })] });
    expect(screen.queryByText('applications.pipeline.overdue')).not.toBeInTheDocument();
  });

  it('the overdue badge is actionable — it surfaces those rows', () => {
    const onShowOverdue = vi.fn();
    renderStrip({ applications: [makeApp({ id: 'a', nextActionAt: NOW - 1 })], onShowOverdue });

    fireEvent.click(screen.getByText('applications.pipeline.overdue'));
    expect(onShowOverdue).toHaveBeenCalledTimes(1);
  });
});
