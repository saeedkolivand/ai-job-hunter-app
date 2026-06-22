import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';

import { GeneratingPanel } from './GeneratingPanel';

// ── Module stubs ──────────────────────────────────────────────────────────────

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

// StepDots: capture the props to assert totalSteps and currentStep.
// The real component is not relevant to the behavioral contract we test here.
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    StepDots: ({ currentStep, totalSteps }: { currentStep: number; totalSteps: number }) => (
      <div
        data-testid={TEST_IDS.documents.stepDots}
        data-current={String(currentStep)}
        data-total={String(totalSteps)}
      />
    ),
  };
});

// ── Default props factory ─────────────────────────────────────────────────────

const noop = () => undefined;

function makeProps(overrides: Partial<Parameters<typeof GeneratingPanel>[0]> = {}) {
  return {
    target: 'both' as const,
    phase: 'idle' as const,
    phaseLabel: 'working…',
    thinking: '',
    output: '',
    onCancel: noop,
    ...overrides,
  };
}

// ── Helper to read StepDots props ─────────────────────────────────────────────

function stepDotsProps(): { current: number; total: number } {
  const el = screen.getByTestId(TEST_IDS.documents.stepDots);
  return {
    current: Number(el.getAttribute('data-current')),
    total: Number(el.getAttribute('data-total')),
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('GeneratingPanel — StepDots mapping (target="both")', () => {
  it('renders 3 total steps for target="both"', () => {
    render(<GeneratingPanel {...makeProps({ target: 'both', phase: 'analyzing' })} />);
    expect(stepDotsProps().total).toBe(3);
  });

  it('phase "analyzing" maps to step 0 (first dot)', () => {
    render(<GeneratingPanel {...makeProps({ target: 'both', phase: 'analyzing' })} />);
    expect(stepDotsProps().current).toBe(0);
  });

  it('phase "resume" maps to step 1 (second dot) for target="both"', () => {
    render(<GeneratingPanel {...makeProps({ target: 'both', phase: 'resume' })} />);
    expect(stepDotsProps().current).toBe(1);
  });

  it('phase "cover" maps to step 2 (third dot) for target="both"', () => {
    render(<GeneratingPanel {...makeProps({ target: 'both', phase: 'cover' })} />);
    expect(stepDotsProps().current).toBe(2);
  });
});

describe('GeneratingPanel — StepDots mapping (single-doc targets)', () => {
  it('renders 2 total steps for target="resume"', () => {
    render(<GeneratingPanel {...makeProps({ target: 'resume', phase: 'analyzing' })} />);
    expect(stepDotsProps().total).toBe(2);
  });

  it('renders 2 total steps for target="cover"', () => {
    render(<GeneratingPanel {...makeProps({ target: 'cover', phase: 'analyzing' })} />);
    expect(stepDotsProps().total).toBe(2);
  });

  it('phase "analyzing" is step 0 for single-doc target', () => {
    render(<GeneratingPanel {...makeProps({ target: 'resume', phase: 'analyzing' })} />);
    expect(stepDotsProps().current).toBe(0);
  });

  it('phase "resume" clamps to step 1 (not 2) for target="resume"', () => {
    render(<GeneratingPanel {...makeProps({ target: 'resume', phase: 'resume' })} />);
    const { current, total } = stepDotsProps();
    expect(current).toBe(1);
    // Must be within range — never out-of-bounds.
    expect(current).toBeLessThan(total);
  });

  it('phase "cover" clamps to step 1 (not 2) for target="cover"', () => {
    render(<GeneratingPanel {...makeProps({ target: 'cover', phase: 'cover' })} />);
    const { current, total } = stepDotsProps();
    expect(current).toBe(1);
    expect(current).toBeLessThan(total);
  });
});

describe('GeneratingPanel — content rendering', () => {
  it('shows the phaseLabel text', () => {
    render(<GeneratingPanel {...makeProps({ phaseLabel: 'autopilot.apply.analyzing' })} />);
    expect(screen.getByText('autopilot.apply.analyzing')).toBeInTheDocument();
  });

  it('renders skeleton bars while output is empty', () => {
    const { container } = render(<GeneratingPanel {...makeProps({ output: '' })} />);
    // Skeleton bars are divs inside the skeleton wrapper; the presence of the
    // wrapper's class is enough to verify the empty-output branch renders.
    expect(container.querySelector('.space-y-2')).toBeInTheDocument();
  });

  it('renders the streaming output text when output is non-empty', () => {
    render(<GeneratingPanel {...makeProps({ output: 'partial resume text…' })} />);
    expect(screen.getByText('partial resume text…')).toBeInTheDocument();
  });

  it('renders the Cancel button', () => {
    render(<GeneratingPanel {...makeProps()} />);
    expect(screen.getByRole('button', { name: /autopilot\.apply\.cancel/i })).toBeInTheDocument();
  });
});
