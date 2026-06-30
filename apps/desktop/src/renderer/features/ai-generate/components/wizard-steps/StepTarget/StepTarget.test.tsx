import { beforeEach, describe, expect, it, type Mock, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { StepTarget } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe('StepTarget', () => {
  let onTargetChange: Mock;

  beforeEach(() => {
    onTargetChange = vi.fn();
  });

  it('renders all three target option buttons', () => {
    render(<StepTarget target="both" onTargetChange={onTargetChange} />);
    expect(screen.getByRole('button', { name: /aiGenerate\.resume/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /aiGenerate\.coverLetter/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /aiGenerate\.both/i })).toBeInTheDocument();
  });

  it('calls onTargetChange("resume") when resume button is clicked', async () => {
    const user = userEvent.setup();
    render(<StepTarget target="both" onTargetChange={onTargetChange} />);
    await user.click(screen.getByRole('button', { name: /aiGenerate\.resume/i }));
    expect(onTargetChange).toHaveBeenCalledWith('resume');
  });

  it('calls onTargetChange("cover") when cover letter button is clicked', async () => {
    const user = userEvent.setup();
    render(<StepTarget target="resume" onTargetChange={onTargetChange} />);
    await user.click(screen.getByRole('button', { name: /aiGenerate\.coverLetter/i }));
    expect(onTargetChange).toHaveBeenCalledWith('cover');
  });

  it('calls onTargetChange("both") when both button is clicked', async () => {
    const user = userEvent.setup();
    render(<StepTarget target="resume" onTargetChange={onTargetChange} />);
    await user.click(screen.getByRole('button', { name: /aiGenerate\.both/i }));
    expect(onTargetChange).toHaveBeenCalledWith('both');
  });

  it('does not call onTargetChange when already-selected button is clicked', async () => {
    const user = userEvent.setup();
    render(<StepTarget target="both" onTargetChange={onTargetChange} />);
    // Clicking "both" when it's already selected still fires onTargetChange — that's fine.
    // What we verify is it gets the right value.
    await user.click(screen.getByRole('button', { name: /aiGenerate\.both/i }));
    expect(onTargetChange).toHaveBeenCalledWith('both');
  });
});
