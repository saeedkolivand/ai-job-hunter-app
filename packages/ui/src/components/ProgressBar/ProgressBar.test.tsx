import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { ProgressBar } from './ProgressBar';

describe('ProgressBar', () => {
  it('shows a rounded percentage label by default', () => {
    render(<ProgressBar value={42.4} />);
    expect(screen.getByText('42%')).toBeInTheDocument();
  });

  it('clamps values below 0 and above 100', () => {
    const { rerender } = render(<ProgressBar value={-10} />);
    expect(screen.getByText('0%')).toBeInTheDocument();
    rerender(<ProgressBar value={150} />);
    expect(screen.getByText('100%')).toBeInTheDocument();
  });

  it('hides the label when showLabel is false', () => {
    render(<ProgressBar value={50} showLabel={false} />);
    expect(screen.queryByText('50%')).not.toBeInTheDocument();
  });
});
