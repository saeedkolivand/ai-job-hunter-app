/**
 * SectionTimeline — the max-depth live checklist.
 *
 * What is pinned here is mostly about what it must NOT do: invent rows for
 * sections the run has not reported, and convey a state by colour alone.
 *
 * jsdom caveat (repo lesson): visibility/appearance assertions are
 * computed-style-only here — rects and offsets are zeroed — so the state a row
 * is in is read off its `data-state` seam and its visible TEXT, never a colour.
 */
import { describe, expect, it } from 'vitest';
import { render, screen, within } from '@testing-library/react';

import type { PipelineSectionStates } from '@/lib/machines/resume-pipeline.machine';

import { SectionTimeline } from './SectionTimeline';

const rows = () => within(screen.getByTestId('section-timeline')).getAllByRole('listitem');

describe('SectionTimeline', () => {
  it('renders nothing at all when the run has reported no sections', () => {
    // Quality depth and every reconnected run land here: the component is
    // rendered unconditionally by its host, so "empty" must mean "invisible",
    // not an empty box captioned "Sections".
    const { container } = render(<SectionTimeline states={{}} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('lists the reported sections in arrival order, which is generation order', () => {
    const states: PipelineSectionStates = {
      summary: 'clean',
      skills: 'done',
      'experience:0': 'generating',
    };
    render(<SectionTimeline states={states} />);
    expect(rows().map((li) => li.getAttribute('data-section'))).toEqual([
      'summary',
      'skills',
      'experience:0',
    ]);
  });

  it('numbers an experience entry from 1 instead of naming a company it was never told', () => {
    // `experience:<i>` indexes the strategy's roster and the company NAME is
    // deliberately not on the (content-free) wire. Guessing one would be the
    // only way this row could lie.
    render(<SectionTimeline states={{ 'experience:2': 'done' }} />);
    expect(screen.getByText('Experience 3')).toBeInTheDocument();
  });

  it('never invents a row for a section that has not reported', () => {
    // The roster is unknowable in advance (it depends on the company plan), so
    // an un-reported section has no row — not a "queued" placeholder.
    render(<SectionTimeline states={{ summary: 'generating' }} />);
    expect(rows()).toHaveLength(1);
    expect(screen.queryByText('Skills')).toBeNull();
    expect(screen.queryByText('Education')).toBeNull();
  });

  it('says each state in words, so colour is never the only carrier', () => {
    // WCAG 1.4.1. Drop the Tag (leaving the tinted icon) and this fails.
    const states: PipelineSectionStates = {
      summary: 'generating',
      skills: 'checking',
      projects: 'clean',
      education: 'needsChanges',
    };
    render(<SectionTimeline states={states} />);
    for (const text of ['Writing', 'Checking', 'No changes needed', 'Needs changes']) {
      expect(screen.getByText(text)).toBeInTheDocument();
    }
  });

  it('gives the list an accessible name and marks its icons decorative', () => {
    render(<SectionTimeline states={{ summary: 'generating' }} />);
    expect(screen.getByRole('group', { name: /sections/i })).toBeInTheDocument();
    // The icon repeats the Tag's text; announcing it twice is noise.
    const [row] = rows();
    expect(row?.querySelector('svg')).toHaveAttribute('aria-hidden', 'true');
  });

  it('renders every state in the ladder without falling back to a raw key', () => {
    const states: PipelineSectionStates = {
      summary: 'queued',
      skills: 'generating',
      'experience:0': 'done',
      'experience:1': 'checking',
      projects: 'repaired',
      education: 'clean',
    };
    render(<SectionTimeline states={states} />);
    expect(rows()).toHaveLength(6);
    // A missing translation would leak `pipeline.section.state.<x>` as text.
    expect(screen.queryByText(/pipeline\.section\./)).toBeNull();
  });
});
