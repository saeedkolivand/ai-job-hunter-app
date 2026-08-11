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

  // `resume_pipeline_run` REJECTS `depth: "max"`. Showing it as pickable would
  // hand the user a validation error; hiding it entirely would claim there are
  // only two depths. Shown + disabled + reasoned is the honest middle.
  it('shows Max but refuses to select it, and says why', async () => {
    const onChange = vi.fn();
    render(<DepthSelector value="quality" onChange={onChange} />);
    const max = screen.getByRole('radio', { name: /max/i });
    expect(max).toHaveAttribute('aria-disabled', 'true');
    expect(max).toHaveAttribute('title', expect.stringMatching(/not available yet/i));
    await userEvent.click(max);
    expect(onChange).not.toHaveBeenCalled();
  });

  it('steps over the disabled option with the arrow keys', async () => {
    const onChange = vi.fn();
    render(<DepthSelector value="quality" onChange={onChange} />);
    screen.getByRole('radio', { name: /quality/i }).focus();
    await userEvent.keyboard('{ArrowRight}');
    // quality → (max is disabled) → wraps to fast, never landing on max.
    expect(onChange).toHaveBeenCalledWith('fast');
    expect(onChange).not.toHaveBeenCalledWith('max');
  });

  it('gives every option a self-describing tooltip', () => {
    render(<DepthSelector value="fast" onChange={vi.fn()} />);
    expect(screen.getByRole('radio', { name: /fast/i })).toHaveAttribute(
      'title',
      expect.stringContaining('One model call')
    );
    expect(screen.getByRole('radio', { name: /quality/i })).toHaveAttribute(
      'title',
      expect.stringContaining('Six stages')
    );
  });

  it('explains all three depths — with honest costs — in the info popover', async () => {
    render(<DepthSelector value="fast" onChange={vi.fn()} />);
    await userEvent.hover(screen.getByRole('button', { name: /what do these depths do/i }));

    expect(await screen.findByText(/1 model call/i)).toBeInTheDocument();
    expect(screen.getByText(/4 model calls plus up to 2 repair rounds/i)).toBeInTheDocument();
    expect(screen.getByText(/\+30–90 seconds/i)).toBeInTheDocument();
    expect(screen.getByText(/7 calls plus up to 9 sequential/i)).toBeInTheDocument();
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

  it('states a surface-level limitation instead of downgrading silently', () => {
    render(
      <DepthSelector value="quality" onChange={vi.fn()} unavailableReason="needs a saved résumé" />
    );
    expect(screen.getByText('needs a saved résumé')).toBeInTheDocument();
  });
});
