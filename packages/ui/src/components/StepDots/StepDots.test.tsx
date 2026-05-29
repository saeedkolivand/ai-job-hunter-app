import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';

import { StepDots } from './StepDots';

describe('StepDots', () => {
  it('renders one dot per step', () => {
    const { container } = render(<StepDots currentStep={1} totalSteps={4} />);
    const dots = (container.firstChild as HTMLElement).children;
    expect(dots).toHaveLength(4);
  });

  it('highlights the active step', () => {
    const { container } = render(<StepDots currentStep={2} totalSteps={3} />);
    const dots = Array.from((container.firstChild as HTMLElement).children);
    expect((dots[2] as HTMLElement).className).toContain('bg-brand');
    expect((dots[0] as HTMLElement).className).toContain('bg-white/15');
  });
});
