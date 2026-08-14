import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    // The announcer interpolates {{step}}/{{state}} — surface them in the
    // returned string so a test can assert the exact announcement built.
    // Every other key (no such params) returns as-is.
    t: (key: string, opts?: Record<string, unknown>) =>
      opts && 'step' in opts && 'state' in opts ? `${key}:${opts.step}:${opts.state}` : key,
  }),
}));

// ThinkingBubble is a streaming display component — stub it so tests can run
// without the real streaming infrastructure, but capture the props GeneratingPanel
// is responsible for wiring (M4/H1: done/defaultExpanded).
const capturedThinkingBubbleProps: { done?: boolean; defaultExpanded?: boolean } = {};
vi.mock('@/components/generation/ThinkingBubble', () => ({
  ThinkingBubble: ({
    thinking,
    done,
    defaultExpanded,
  }: {
    thinking: string;
    done?: boolean;
    defaultExpanded?: boolean;
  }) => {
    capturedThinkingBubbleProps.done = done;
    capturedThinkingBubbleProps.defaultExpanded = defaultExpanded;
    return <div data-testid={TEST_IDS.documents.thinkingBubble}>{thinking}</div>;
  },
}));

import { GeneratingPanel } from './GeneratingPanel';

const noop = () => undefined;

function makeProps(overrides: Partial<Parameters<typeof GeneratingPanel>[0]> = {}) {
  return {
    currentStep: 0,
    stageLabel: '',
    thinking: '',
    output: '',
    streamingTarget: 'resume' as const,
    onCancel: noop,
    ...overrides,
  };
}

beforeEach(() => {
  delete capturedThinkingBubbleProps.done;
  delete capturedThinkingBubbleProps.defaultExpanded;
});

describe('GeneratingPanel — the 4-step checklist', () => {
  it('renders all four steps and labels', () => {
    render(<GeneratingPanel {...makeProps()} />);
    for (const key of ['analyze', 'generate', 'validate', 'humanize']) {
      expect(screen.getByText(`pipeline.step.${key}.label`)).toBeInTheDocument();
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

  // M2: the active row shows ONLY the stage caption (not the description too);
  // every OTHER row shows its description, never the stage caption.
  it('shows the stage caption ONLY on the active row, and the description only on non-active rows', () => {
    render(
      <GeneratingPanel {...makeProps({ currentStep: 2, stageLabel: 'Checking the result' })} />
    );
    const rows = screen.getAllByRole('listitem');
    expect(rows[2]).toHaveTextContent('Checking the result');
    expect(rows[2]).not.toHaveTextContent('pipeline.step.validate.description');
    expect(rows[0]).toHaveTextContent('pipeline.step.analyze.description');
    expect(rows[0]).not.toHaveTextContent('Checking the result');
    expect(rows[1]).toHaveTextContent('pipeline.step.generate.description');
    expect(rows[3]).toHaveTextContent('pipeline.step.humanize.description');
  });

  // CR-6: the active row NEVER shows `.description` regardless of
  // `stageLabel` (see the test above — that's M2, structural), so asserting
  // its absence here was tautologically true either way and never proved
  // this behavior. The caption (`{stageLabel} · {elapsedLabel}`) always
  // includes the elapsed mm:ss — its absence is the real signal that
  // NOTHING rendered for an empty stageLabel.
  it('omits the caption when no stage label is given yet (active row shows nothing extra)', () => {
    render(<GeneratingPanel {...makeProps({ currentStep: 0, stageLabel: '' })} />);
    const rows = screen.getAllByRole('listitem');
    expect(rows[0]?.textContent).not.toMatch(/\d:\d{2}/);
  });

  // H8: an aria-hidden icon is the only visual state cue — sr-only text must
  // carry the state so a screen reader doesn't hear identical rows.
  it('carries an sr-only state word per row (done/active/pending)', () => {
    render(<GeneratingPanel {...makeProps({ currentStep: 1 })} />);
    const rows = screen.getAllByRole('listitem');
    expect(rows[0]).toHaveTextContent('pipeline.step.state.done');
    expect(rows[1]).toHaveTextContent('pipeline.step.state.active');
    expect(rows[2]).toHaveTextContent('pipeline.step.state.pending');
    expect(rows[3]).toHaveTextContent('pipeline.step.state.pending');
  });

  // H8: one announcer utterance per step TRANSITION.
  it('announces a step transition via the sr-only live region', () => {
    const { rerender } = render(<GeneratingPanel {...makeProps({ currentStep: 0 })} />);
    const status = screen.getByRole('status', { hidden: true });
    expect(status).toHaveTextContent('');

    rerender(<GeneratingPanel {...makeProps({ currentStep: 1 })} />);
    expect(status).toHaveTextContent(
      'pipeline.step.announce:pipeline.step.generate.label:pipeline.step.state.active'
    );
  });

  it('does not re-announce when currentStep is unchanged across a re-render', () => {
    const { rerender } = render(
      <GeneratingPanel {...makeProps({ currentStep: 1, stageLabel: 'a' })} />
    );
    const status = screen.getByRole('status', { hidden: true });
    const first = status.textContent;
    rerender(<GeneratingPanel {...makeProps({ currentStep: 1, stageLabel: 'b' })} />);
    expect(status.textContent).toBe(first);
  });

  // Terminal-state announcement: `PIPELINE_STEP_KEYS[currentStep]` is
  // undefined once currentStep reaches the step count (every step just
  // finished) — previously a silent no-op, so a screen-reader user heard
  // every step START and never heard the run finish.
  it('announces pipeline.step.allDone once currentStep reaches the step count', () => {
    const { rerender } = render(<GeneratingPanel {...makeProps({ currentStep: 3 })} />);
    const status = screen.getByRole('status', { hidden: true });
    expect(status).not.toHaveTextContent('pipeline.step.allDone');

    rerender(<GeneratingPanel {...makeProps({ currentStep: 4 })} />);
    expect(status).toHaveTextContent('pipeline.step.allDone');
  });

  it('still applies the one-utterance-per-transition guard at the terminal step', () => {
    const { rerender } = render(<GeneratingPanel {...makeProps({ currentStep: 4 })} />);
    const status = screen.getByRole('status', { hidden: true });
    const first = status.textContent;
    rerender(<GeneratingPanel {...makeProps({ currentStep: 4, stageLabel: 'irrelevant' })} />);
    expect(status.textContent).toBe(first);
  });

  // H6/M5: the active row's label AND its stage caption use the bare
  // `text-brand-soft` class (no opacity suffix — an opacity-suffixed variant
  // never gets the light-scheme remap, which is the regression this guards).
  it('styles the active row label and caption with bare text-brand-soft (no /NN suffix)', () => {
    render(
      <GeneratingPanel {...makeProps({ currentStep: 1, stageLabel: 'Writing the résumé' })} />
    );
    const rows = screen.getAllByRole('listitem');
    const activeRow = rows[1] as HTMLElement;
    const label = activeRow.querySelector('.text-brand-soft.font-medium');
    expect(label).not.toBeNull();
    const caption = Array.from(activeRow.querySelectorAll('span')).find(
      (el) =>
        el.className.includes('text-brand-soft') && el.textContent?.includes('Writing the résumé')
    );
    expect(caption).toBeDefined();
    expect(caption?.className).not.toMatch(/text-brand-soft\/\d/);
  });
});

describe('GeneratingPanel — elapsed-time caption (H3)', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('starts at 0:00 for a freshly-active step and ticks upward every second', () => {
    render(<GeneratingPanel {...makeProps({ currentStep: 0, stageLabel: 'Reading' })} />);
    expect(screen.getByText(/0:00/)).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(screen.getByText(/0:03/)).toBeInTheDocument();
  });

  it('resets to 0:00 when the active step changes', () => {
    const { rerender } = render(
      <GeneratingPanel {...makeProps({ currentStep: 0, stageLabel: 'Reading' })} />
    );
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(screen.getByText(/0:05/)).toBeInTheDocument();

    rerender(<GeneratingPanel {...makeProps({ currentStep: 1, stageLabel: 'Writing' })} />);
    expect(screen.getByText(/0:00/)).toBeInTheDocument();
  });
});

describe('GeneratingPanel — ThinkingBubble wiring (H1/M4)', () => {
  it('passes defaultExpanded=false (collapsed by default in this tight panel)', () => {
    render(<GeneratingPanel {...makeProps()} />);
    expect(capturedThinkingBubbleProps.defaultExpanded).toBe(false);
  });

  it('passes done=false before validate (currentStep < 2)', () => {
    render(<GeneratingPanel {...makeProps({ currentStep: 1 })} />);
    expect(capturedThinkingBubbleProps.done).toBe(false);
  });

  it('passes done=true once validate starts (currentStep >= 2) — the model has stopped reasoning by then', () => {
    render(<GeneratingPanel {...makeProps({ currentStep: 2 })} />);
    expect(capturedThinkingBubbleProps.done).toBe(true);
  });
});

describe('GeneratingPanel — streaming document header (M3)', () => {
  it('labels the pane "résumé" while the résumé is streaming', () => {
    render(<GeneratingPanel {...makeProps({ streamingTarget: 'resume' })} />);
    expect(screen.getByText('autopilot.apply.target.resume')).toBeInTheDocument();
    expect(screen.queryByText('autopilot.apply.target.cover')).not.toBeInTheDocument();
  });

  // N3: was text-foreground/45 (4.22:1 dark, below the AA text floor) — the
  // repo's documented sub-14px floor is /70 (8.04:1 dark / 6.20:1 light).
  it('renders the header at the AA-safe text-foreground/70, not /45', () => {
    render(<GeneratingPanel {...makeProps({ streamingTarget: 'resume' })} />);
    const header = screen.getByText('autopilot.apply.target.resume');
    expect(header.className).toContain('text-foreground/70');
    expect(header.className).not.toContain('text-foreground/45');
  });

  it('labels the pane "cover letter" once the letter starts streaming', () => {
    render(<GeneratingPanel {...makeProps({ streamingTarget: 'cover' })} />);
    expect(screen.getByText('autopilot.apply.target.cover')).toBeInTheDocument();
    expect(screen.queryByText('autopilot.apply.target.resume')).not.toBeInTheDocument();
  });
});

describe('GeneratingPanel — content rendering', () => {
  it('renders skeleton bars while output is empty', () => {
    const { container } = render(<GeneratingPanel {...makeProps({ output: '' })} />);
    expect(container.querySelectorAll('.animate-skeleton').length).toBeGreaterThan(0);
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

  it('renders the Cancel button with bare text-red-300 (M9: no /80 opacity suffix)', () => {
    render(<GeneratingPanel {...makeProps()} />);
    const button = screen.getByRole('button', { name: /autopilot\.apply\.cancel/i });
    expect(button).toBeInTheDocument();
    expect(button.className).toContain('text-red-300');
    expect(button.className).not.toMatch(/text-red-300\/\d/);
  });
});
