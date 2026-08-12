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

describe('GeneratingPanel — reasoning-heavy hint', () => {
  // A reasoning model logged 1,039,821 chars of reasoning against 45,416 of
  // answer and took 8–13 minutes per document, where a non-reasoning model did
  // the same work in 12–23 seconds. The panel showed the same "this can take a
  // moment" the whole time, so the run was indistinguishable from a hang.

  it('keeps the normal hint on an ordinary run', () => {
    render(<GeneratingPanel {...makeProps({ thinking: 'brief thought', output: 'Dear team,' })} />);
    expect(screen.getByText('autopilot.apply.generatingHint')).toBeInTheDocument();
  });

  it('switches to the reasoning-heavy hint when reasoning dwarfs the document', () => {
    render(
      <GeneratingPanel {...makeProps({ thinking: 'x'.repeat(20_000), output: 'Dear team,' })} />
    );
    expect(screen.getByText('autopilot.apply.reasoningHeavyHint')).toBeInTheDocument();
    expect(screen.queryByText('autopilot.apply.generatingHint')).not.toBeInTheDocument();
  });

  it('does not fire on a long run whose OUTPUT kept pace', () => {
    // Both large: the model is producing a document, not just thinking. The
    // ratio is the signal, not the absolute size — otherwise every long
    // generation would show a warning that means nothing.
    render(
      <GeneratingPanel
        {...makeProps({ thinking: 'x'.repeat(20_000), output: 'y'.repeat(20_000) })}
      />
    );
    expect(screen.getByText('autopilot.apply.generatingHint')).toBeInTheDocument();
  });

  it('pins both thresholds exactly', () => {
    // 8,000 chars and a 5:1 ratio are the two numbers the behaviour rests on.
    // Without boundary cases either could be widened away silently.
    const hint = () => screen.queryByText('autopilot.apply.reasoningHeavyHint');

    // Just under the floor → no hint, even at an enormous ratio.
    const { unmount } = render(
      <GeneratingPanel {...makeProps({ thinking: 'x'.repeat(7_999), output: '' })} />
    );
    expect(hint()).not.toBeInTheDocument();
    unmount();

    // At the floor but exactly AT the ratio (not above it) → no hint.
    const second = render(
      <GeneratingPanel {...makeProps({ thinking: 'x'.repeat(8_000), output: 'y'.repeat(1_600) })} />
    );
    expect(hint()).not.toBeInTheDocument();
    second.unmount();

    // At the floor and one char past the ratio → hint.
    render(
      <GeneratingPanel {...makeProps({ thinking: 'x'.repeat(8_001), output: 'y'.repeat(1_600) })} />
    );
    expect(hint()).toBeInTheDocument();
  });

  it('does not fire on a short burst of reasoning', () => {
    // Under the floor: early in ANY reasoning run the ratio is briefly huge
    // (thinking streams before the document does), and flashing the hint for a
    // second on every run would train the user to ignore it.
    render(<GeneratingPanel {...makeProps({ thinking: 'x'.repeat(2_000), output: '' })} />);
    expect(screen.getByText('autopilot.apply.generatingHint')).toBeInTheDocument();
  });
});

describe('GeneratingPanel — staged (quality-depth) run', () => {
  it('drives the dots off the pipeline stage counter, not the document phases', () => {
    render(
      <GeneratingPanel
        {...makeProps({
          target: 'resume',
          pipelineStep: { current: 3, total: 6 },
          stageLabel: 'Writing the résumé',
        })}
      />
    );
    // The fast path would show 2 dots for a resume-only run; the staged run
    // shows the pipeline's own six.
    expect(stepDotsProps()).toEqual({ current: 3, total: 6 });
  });

  it('names the stage and its position', () => {
    render(
      <GeneratingPanel
        {...makeProps({
          pipelineStep: { current: 4, total: 6 },
          stageLabel: 'Checking the result',
        })}
      />
    );
    expect(screen.getByText('Checking the result')).toBeInTheDocument();
    expect(screen.getByText('pipeline.stageCounter')).toBeInTheDocument();
  });

  it('falls back to the document phases when a reconnected run has no length', () => {
    // A run recovered from its persisted trail knows which stage it reached but
    // not how many the pipeline has — an empty dot row would read as no progress.
    render(
      <GeneratingPanel
        {...makeProps({
          target: 'both',
          phase: 'analyzing',
          pipelineStep: { current: 0, total: 0 },
        })}
      />
    );
    expect(stepDotsProps()).toEqual({ current: 0, total: 3 });
  });

  it('leaves the fast path untouched when no staged progress is supplied', () => {
    render(<GeneratingPanel {...makeProps({ target: 'both', phase: 'resume' })} />);
    expect(stepDotsProps()).toEqual({ current: 1, total: 3 });
    expect(screen.queryByText('pipeline.stageCounter')).not.toBeInTheDocument();
    expect(screen.getByText('working…')).toBeInTheDocument();
  });

  it('never lets the dot index run past the last dot', () => {
    render(<GeneratingPanel {...makeProps({ pipelineStep: { current: 6, total: 6 } })} />);
    expect(stepDotsProps()).toEqual({ current: 5, total: 6 });
  });
});
