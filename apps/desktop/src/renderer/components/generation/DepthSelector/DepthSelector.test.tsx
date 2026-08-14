import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { DepthSelector } from './DepthSelector';

describe('DepthSelector', () => {
  it('offers all three depths as radios', () => {
    render(<DepthSelector value="fast" onChange={vi.fn()} />);
    const radios = screen.getAllByRole('radio');
    expect(radios).toHaveLength(3);
    expect(screen.getByRole('radio', { name: /fast/i })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /quality/i })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /max/i })).toBeInTheDocument();
  });

  it('selects a runnable depth on click', async () => {
    const onChange = vi.fn();
    render(<DepthSelector value="fast" onChange={onChange} />);
    await userEvent.click(screen.getByRole('radio', { name: /quality/i }));
    expect(onChange).toHaveBeenCalledWith('quality');
  });

  // Phase 4: `resume_pipeline_run` routes `depth: "max"` to the section-wise
  // pipeline instead of rejecting it, so the option is selectable and no longer
  // carries a "not available" title. Delete `'max'` from
  // RUNNABLE_GENERATION_DEPTHS and both assertions here fail.
  it('offers Max as a real choice now that the pipeline runs it', async () => {
    const onChange = vi.fn();
    render(<DepthSelector value="quality" onChange={onChange} />);
    const max = screen.getByRole('radio', { name: /max/i });
    expect(max).not.toHaveAttribute('aria-disabled');
    expect(max).toHaveAttribute('title', expect.stringMatching(/eight stages/i));
    await userEvent.click(max);
    expect(onChange).toHaveBeenCalledWith('max');
  });

  it('reaches every depth with the arrow keys — none is stepped over', async () => {
    const onChange = vi.fn();
    render(<DepthSelector value="quality" onChange={onChange} />);
    screen.getByRole('radio', { name: /quality/i }).focus();
    await userEvent.keyboard('{ArrowRight}');
    expect(onChange).toHaveBeenCalledWith('max');
  });

  it('gives every option a self-describing tooltip', () => {
    render(<DepthSelector value="fast" onChange={vi.fn()} />);
    expect(screen.getByRole('radio', { name: /fast/i })).toHaveAttribute(
      'title',
      expect.stringContaining('One model call')
    );
    expect(screen.getByRole('radio', { name: /quality/i })).toHaveAttribute(
      'title',
      expect.stringContaining('Up to eight stages')
    );
    expect(screen.getByRole('radio', { name: /max/i })).toHaveAttribute(
      'title',
      expect.stringContaining('Eight stages')
    );
  });

  it('explains all three depths — with honest costs — in the info popover', async () => {
    render(<DepthSelector value="fast" onChange={vi.fn()} />);
    await userEvent.hover(screen.getByRole('button', { name: /what do these depths do/i }));

    expect(await screen.findByText(/1 model call/i)).toBeInTheDocument();
    expect(screen.getByText(/4–5 model calls/i)).toBeInTheDocument();
    expect(screen.getByText(/up to 2 clean-up passes/i)).toBeInTheDocument();
    expect(screen.getByText(/\+30–90 seconds/i)).toBeInTheDocument();
    expect(screen.getByText(/one per section \(up to 12\)/i)).toBeInTheDocument();
    // The max run's own wall clock, so "how long can this possibly take" has an
    // answer in the popover rather than only in the backend's deadline.
    expect(screen.getByText(/about two hours/i)).toBeInTheDocument();
    // What the integrity report adds, in plain language.
    expect(screen.getByText(/rules, not an AI score/i)).toBeInTheDocument();
  });

  describe('soft small-model warning', () => {
    it('warns — and never blocks — at quality depth on a small local model', async () => {
      const onChange = vi.fn();
      render(<DepthSelector value="quality" onChange={onChange} smallModel />);
      expect(
        screen.getByText(/small local models often get that shape wrong/i)
      ).toBeInTheDocument();
      // Still selectable: the plan's decision is honest copy, never a block.
      await userEvent.click(screen.getByRole('radio', { name: /fast/i }));
      expect(onChange).toHaveBeenCalledWith('fast');
    });

    it('stays quiet at fast depth — the JSON stages are what it warns about', () => {
      render(<DepthSelector value="fast" onChange={vi.fn()} smallModel />);
      expect(screen.queryByText(/small local models/i)).not.toBeInTheDocument();
    });
  });

  describe('a surface that cannot run a staged depth says so', () => {
    it.each(['quality', 'max'] as const)(
      'states the limitation at %s depth instead of downgrading silently',
      (value) => {
        // Both staged depths need the SAME server-resolved inputs, so a surface
        // that cannot supply them cannot run either. Narrow this back to
        // `value === 'quality'` and the max case fails — with a `max` selection
        // silently claiming it is about to run.
        render(
          <DepthSelector
            value={value}
            onChange={vi.fn()}
            unavailableReason="needs a saved résumé"
          />
        );
        expect(screen.getByText('needs a saved résumé')).toBeInTheDocument();
      }
    );

    it('stays quiet at fast depth — that one runs from anywhere', () => {
      render(
        <DepthSelector value="fast" onChange={vi.fn()} unavailableReason="needs a saved résumé" />
      );
      expect(screen.queryByText('needs a saved résumé')).not.toBeInTheDocument();
    });
  });
});
