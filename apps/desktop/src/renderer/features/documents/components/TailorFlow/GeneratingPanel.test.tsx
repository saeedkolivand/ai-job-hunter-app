import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ThinkingBubble is a streaming display component — stub it so tests can run
// without the real streaming infrastructure.
vi.mock('@/components/generation/ThinkingBubble', () => ({
  ThinkingBubble: ({ thinking }: { thinking: string }) => (
    <div data-testid={TEST_IDS.documents.thinkingBubble}>{thinking}</div>
  ),
}));

import { GeneratingPanel } from './GeneratingPanel';

const noop = () => undefined;

function makeProps(overrides: Partial<Parameters<typeof GeneratingPanel>[0]> = {}) {
  return {
    currentStep: 0,
    stageLabel: '',
    thinking: '',
    output: '',
    onCancel: noop,
    ...overrides,
  };
}

describe('GeneratingPanel — the 4-step checklist', () => {
  it('renders all four steps, labels and descriptions', () => {
    render(<GeneratingPanel {...makeProps()} />);
    for (const key of ['analyze', 'generate', 'validate', 'humanize']) {
      expect(screen.getByText(`pipeline.step.${key}.label`)).toBeInTheDocument();
      expect(screen.getByText(`pipeline.step.${key}.description`)).toBeInTheDocument();
    }
  });

  it('marks every step before currentStep as done (checkmark, not a number)', () => {
    const { container } = render(<GeneratingPanel {...makeProps({ currentStep: 2 })} />);
    const rows = container.querySelectorAll('li');
    // Steps 0 and 1 are done — their number badge is replaced by a check icon,
    // not the literal digit "1"/"2".
    expect(rows[0]?.textContent).not.toContain('1');
    expect(rows[1]?.textContent).not.toContain('2');
  });

  it('marks the current step aria-current="step"', () => {
    render(<GeneratingPanel {...makeProps({ currentStep: 1 })} />);
    const rows = screen.getAllByRole('listitem');
    expect(rows[1]).toHaveAttribute('aria-current', 'step');
    expect(rows[0]).not.toHaveAttribute('aria-current');
    expect(rows[2]).not.toHaveAttribute('aria-current');
  });

  it('shows the numbered badge for a step that has not started yet', () => {
    render(<GeneratingPanel {...makeProps({ currentStep: 0 })} />);
    const rows = screen.getAllByRole('listitem');
    // Step 4 ("humanize", index 3) is neither done nor active — its badge is
    // its own 1-based number.
    expect(rows[3]?.textContent).toContain('4');
  });

  it('shows the current stage caption only on the active row', () => {
    render(
      <GeneratingPanel {...makeProps({ currentStep: 2, stageLabel: 'Checking the result' })} />
    );
    const rows = screen.getAllByRole('listitem');
    expect(rows[2]).toHaveTextContent('Checking the result');
    expect(rows[0]).not.toHaveTextContent('Checking the result');
    expect(rows[1]).not.toHaveTextContent('Checking the result');
    expect(rows[3]).not.toHaveTextContent('Checking the result');
  });

  it('omits the caption when no stage label is given yet', () => {
    const { container } = render(
      <GeneratingPanel {...makeProps({ currentStep: 0, stageLabel: '' })} />
    );
    expect(container.querySelector('.tracking-\\[0\\.14em\\]')).not.toBeInTheDocument();
  });
});

describe('GeneratingPanel — content rendering', () => {
  it('renders skeleton bars while output is empty', () => {
    const { container } = render(<GeneratingPanel {...makeProps({ output: '' })} />);
    expect(container.querySelector('.space-y-2')).toBeInTheDocument();
  });

  it('renders the streaming output text when output is non-empty', () => {
    render(<GeneratingPanel {...makeProps({ output: 'partial resume text…' })} />);
    expect(screen.getByText('partial resume text…')).toBeInTheDocument();
  });

  it('renders the thinking bubble', () => {
    render(<GeneratingPanel {...makeProps({ thinking: 'considering the evidence' })} />);
    expect(screen.getByTestId(TEST_IDS.documents.thinkingBubble)).toHaveTextContent(
      'considering the evidence'
    );
  });

  it('renders the Cancel button', () => {
    render(<GeneratingPanel {...makeProps()} />);
    expect(screen.getByRole('button', { name: /autopilot\.apply\.cancel/i })).toBeInTheDocument();
  });
});
