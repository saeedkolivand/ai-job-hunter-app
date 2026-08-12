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

  it('renders an unparseable entry index as "?" rather than NaN', () => {
    // The runtime grammar is enforced at the emitter, but the TYPE is a
    // template literal that accepts `experience:NaN`. Drop the
    // `Number.isFinite` guard and this renders "Experience NaN".
    render(<SectionTimeline states={{ 'experience:NaN': 'done' } as PipelineSectionStates} />);
    expect(screen.getByText('Experience ?')).toBeInTheDocument();
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

  it('stays a real list, so AT can say "list, N items" and navigate it', () => {
    // It was `role="group"`, which silently dropped list semantics for a
    // rationale (live-region noise) that role never controlled. Put the
    // `role="group"` back and this fails.
    render(<SectionTimeline states={{ summary: 'generating', skills: 'queued' }} />);
    const list = screen.getByRole('list', { name: /sections/i });
    expect(within(list).getAllByRole('listitem')).toHaveLength(2);
  });

  it('marks its icons decorative', () => {
    render(<SectionTimeline states={{ summary: 'generating' }} />);
    // The icon repeats the Tag's text; announcing it twice is noise.
    const [row] = rows();
    expect(row?.querySelector('svg')).toHaveAttribute('aria-hidden', 'true');
  });

  // ── Liveness ──────────────────────────────────────────────────────────────
  //
  // The host's stage caption is FROZEN for the whole sections stage: a section
  // event carries the stage's index/total, so "Step 4 of 8" and the stage name
  // are identical for every section. Both the announcer and the counter below
  // exist because of that — without them the run's longest phase is silent for
  // a screen-reader user and looks stalled for everyone else.
  describe('liveness during the sections stage', () => {
    const announcer = () => screen.getByRole('status');

    it('announces the section that changed, once per transition', () => {
      const { rerender } = render(<SectionTimeline states={{ summary: 'generating' }} />);
      expect(announcer()).toHaveTextContent('Summary — Writing');

      rerender(<SectionTimeline states={{ summary: 'done', skills: 'generating' }} />);
      expect(announcer()).toHaveTextContent('Skills — Writing');
    });

    // Drop the `previous.current[key] === state` guard (announce off the whole
    // map on every render) and this fails: `skills` is the LAST entry but
    // `summary` is what moved, so the utterance would name the wrong section
    // and would repeat on a render that changed nothing.
    it('names the section that MOVED, not the last one in the map', () => {
      const before = { summary: 'generating', skills: 'generating' } as PipelineSectionStates;
      const { rerender } = render(<SectionTimeline states={before} />);
      rerender(<SectionTimeline states={{ ...before, summary: 'done' }} />);
      expect(announcer()).toHaveTextContent('Summary — Written');
      expect(announcer()).not.toHaveTextContent('Skills');
    });

    // `validate`'s start moves every written section at once. Eight utterances
    // for one stage boundary is the noise the milestone pattern exists to
    // avoid, so the batch collapses to its last member.
    it('collapses a batch transition into one utterance', () => {
      const before = { summary: 'done', skills: 'done' } as PipelineSectionStates;
      const { rerender } = render(<SectionTimeline states={before} />);
      rerender(<SectionTimeline states={{ summary: 'checking', skills: 'checking' }} />);
      expect(announcer()).toHaveTextContent('Skills — Checking');
    });

    it('counts written sections against what it has SEEN, never against a total', () => {
      // "3 of 9" would be a number this build made up — the roster is not on
      // the wire. Remove the counter and this fails.
      render(
        <SectionTimeline
          states={{ summary: 'clean', skills: 'done', 'experience:0': 'generating' }}
        />
      );
      expect(screen.getByText('2 of 3 sections written so far')).toBeInTheDocument();
    });

    it('does not count a section that is still being written', () => {
      render(<SectionTimeline states={{ summary: 'generating' }} />);
      expect(screen.getByText('0 of 1 section written so far')).toBeInTheDocument();
    });
  });

  it('renders every state in the ladder with its resolved label', () => {
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

    // The RESOLVED text, not just the absence of a raw key: asserting only
    // `queryByText(/pipeline\.section\./) === null` also passes when the label
    // is never rendered at all, which is exactly the regression worth catching.
    for (const label of [
      'Waiting',
      'Writing',
      'Written',
      'Checking',
      'Repaired',
      'No changes needed',
    ]) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
    // …and the remaining rung still resolves, on a row of its own.
    render(<SectionTimeline states={{ summary: 'needsChanges' }} />);
    expect(screen.getByText('Needs changes')).toBeInTheDocument();

    expect(screen.queryByText(/pipeline\.section\./)).toBeNull();
  });
});
