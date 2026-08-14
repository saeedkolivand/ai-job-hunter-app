import type { ComponentProps } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type * as AjhTranslations from '@ajh/translations';

import { ResultsPanel } from './ResultsPanel';

// Echo every key verbatim — no i18next runtime needed in jsdom. Only
// `useTranslation` is replaced; every other export stays real, so adding one
// doesn't silently break this file.
vi.mock('@ajh/translations', async (importOriginal) => ({
  ...(await importOriginal<typeof AjhTranslations>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));

// The viewer itself is covered by GenerationOutput.test.tsx; here it is a marker
// so the assertions are about ResultsPanel's own layout shape only.
vi.mock('./GenerationOutput', () => ({
  GenerationOutput: () => <div data-testid="generation-output-stub" />,
}));

function makeProps(): ComponentProps<typeof ResultsPanel> {
  return {
    target: 'both',
    jobDesc: 'Full job description text',
    onJobDescChange: vi.fn(),
    hasDesc: true,
    fetchingDesc: false,
    jobUrl: 'https://example.com/job',
    jobAdSummary: {
      summary: '',
      generating: false,
      error: null,
      generate: vi.fn(),
      language: 'en',
      setLanguage: vi.fn(),
    },
    activeOut: 'resume',
    setActiveOut: vi.fn(),
    templateId: 'classic',
    atsMode: false,
    accent: undefined,
    letterLayoutId: undefined,
    onTemplateChange: vi.fn(),
    onAtsModeChange: vi.fn(),
    onAccentChange: vi.fn(),
    onLetterLayoutChange: vi.fn(),
    output: 'Generated resume content',
    onEdit: vi.fn(),
    meta: null,
    copied: false,
    onCopy: vi.fn(),
    exportOpen: false,
    setExportOpen: vi.fn(),
    onExport: vi.fn(),
    runState: 'done',
    onRegenerate: vi.fn(),
    onEditSettings: vi.fn(),
  };
}

// Regression guard for the "document viewer header scrolls away" bug: this panel
// used to wrap GenerationOutput in an `overflow-y-auto` body, so IT scrolled the
// whole viewer — pinned header included. The scroll boundary belongs inside
// GenerationOutput (on its tabpanel); this body must stay height-bounded only.
describe('ResultsPanel layout', () => {
  const SCROLLS = /overflow-(?:y-)?(?:auto|scroll)/;

  it('does not scroll the viewer — no scroll container above GenerationOutput', () => {
    const { container } = render(<ResultsPanel {...makeProps()} />);

    for (
      let el = screen.getByTestId('generation-output-stub').parentElement;
      el !== null && el !== container;
      el = el.parentElement
    ) {
      expect(el.className).not.toMatch(SCROLLS);
    }
  });

  it('bounds the viewer body so it can never grow past the panel', () => {
    render(<ResultsPanel {...makeProps()} />);

    const body = screen.getByTestId('generation-output-stub').parentElement;
    expect(body).not.toBeNull();
    expect(body?.className).toContain('min-h-0');
    expect(body?.className).toContain('flex-1');
  });

  it('keeps the regenerate / edit-settings footer pinned below the body', () => {
    render(<ResultsPanel {...makeProps()} />);

    const footer = screen
      .getByRole('button', { name: 'autopilot.apply.wizard.results.regenerate' })
      .closest('div');
    expect(footer).not.toBeNull();
    expect(footer?.className).toContain('shrink-0');
    expect(footer?.contains(screen.getByTestId('generation-output-stub'))).toBe(false);
  });

  it('forwards the footer actions to their callbacks', () => {
    const props = makeProps();
    render(<ResultsPanel {...props} />);

    screen.getByRole('button', { name: 'autopilot.apply.wizard.results.regenerate' }).click();
    screen.getByRole('button', { name: 'autopilot.apply.wizard.results.edit' }).click();

    expect(props.onRegenerate).toHaveBeenCalledTimes(1);
    expect(props.onEditSettings).toHaveBeenCalledTimes(1);
  });
});

describe('ResultsPanel — status banner', () => {
  it('shows nothing extra on a clean finish (runState=done)', () => {
    render(<ResultsPanel {...makeProps()} runState="done" />);
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows the needsReview banner with the unresolved claim count', () => {
    render(
      <ResultsPanel
        {...makeProps()}
        runState="needsReview"
        pipelineReview={{
          documentText: 'Built the pipeline with 250 users impact.',
          sections: [],
          fabrications: [
            { issueKey: 'a#0', code: 'a', evidence: '250 users' },
            { issueKey: 'a#1', code: 'a', evidence: 'never in the document' },
          ],
        }}
      />
    );
    expect(screen.getByRole('status')).toHaveTextContent(
      'autopilot.apply.wizard.results.needsReviewTitle'
    );
  });

  it('shows the failed banner with the error detail text', () => {
    render(<ResultsPanel {...makeProps()} runState="error" error="Model timed out" />);
    const alert = screen.getByRole('alert');
    expect(alert).toHaveTextContent('autopilot.apply.wizard.results.failedTitle');
    expect(alert).toHaveTextContent('Model timed out');
  });

  it('shows the cancelled hint without an alert/status role', () => {
    render(<ResultsPanel {...makeProps()} runState="cancelled" />);
    expect(screen.getByText('autopilot.apply.wizard.results.cancelledHint')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows the stopped-reason suffix when the run carries one', () => {
    render(<ResultsPanel {...makeProps()} runState="cancelled" stoppedReason="cancelled" />);
    expect(screen.getByText('pipeline.stopped.cancelled')).toBeInTheDocument();
  });
});

describe('ResultsPanel — run history', () => {
  it('renders nothing extra when there is no run history', () => {
    render(<ResultsPanel {...makeProps()} runs={[]} />);
    expect(screen.queryByText('pipeline.runs.title')).not.toBeInTheDocument();
  });
});
