/**
 * JobAdView — unit tests for the truncation/paste UX fix + a11y wiring.
 *
 * Covers:
 *   1. Default tab selection — source when no description or truncated; summary otherwise.
 *   2. TextArea always present on the source tab (even with empty/failed/no-description states).
 *   3. onJobDescChange fires when the textarea value changes.
 *   4. Truncation hint visible iff description ends with ellipsis.
 *   5. ExternalLink "view job" rendered only when jobUrl is provided.
 *   6. TextArea a11y: short aria-label (tab key, NOT editHelper sentence) + aria-describedby wiring.
 *
 * Strategy:
 *  - `@ajh/translations` returns keys as-is (deterministic assertions).
 *  - `@ajh/ui` primitives that this component uses render their child content /
 *    pass through props we assert on; we do NOT stub them — real primitives
 *    catch future API changes early.
 *  - `@/components/ui/ExternalLink` and `@/lib/generate` are real imports
 *    (no network, pure).
 *  - noUncheckedIndexedAccess: all array index accesses are guarded.
 */

import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── ModelSelector — self-contained store-driven picker; stub for unit tests ────
// The real component pulls React Query + AppClient; a stub that renders a
// sentinel element is sufficient to assert it mounts on the summary tab.

// className is forwarded so the containment test below can assert it — the
// real component applies `className` to its own root div.
vi.mock('@/components/ui/ModelSelector', () => ({
  ModelSelector: ({ className }: { className?: string }) => (
    <div data-testid="model-selector-stub" className={className} />
  ),
}));

// ── ExternalLink — thin anchor wrapper, no special provider needed ─────────────

vi.mock('@/components/ui/ExternalLink', () => ({
  ExternalLink: ({
    href,
    children,
    ...rest
  }: { href: string; children: React.ReactNode } & React.HTMLAttributes<HTMLAnchorElement>) => (
    <a href={href} {...rest}>
      {children}
    </a>
  ),
}));

// ── OUTPUT_LANGUAGES — only the shape matters; stub to a minimal list ──────────

vi.mock('@/lib/generate', () => ({
  OUTPUT_LANGUAGES: [{ code: 'en', endonym: 'English' }],
}));

// ── Import component AFTER all mocks ─────────────────────────────────────────

import { JobAdView } from './JobAdView';

// ── Helpers ───────────────────────────────────────────────────────────────────

function makeProps(overrides: Partial<Parameters<typeof JobAdView>[0]> = {}) {
  return {
    jobDesc: 'Full job description with enough text.',
    onJobDescChange: vi.fn(),
    summary: '',
    generating: false,
    error: null,
    onGenerateSummary: vi.fn(),
    language: 'en',
    onLanguageChange: vi.fn(),
    hasDesc: true,
    fetchingDesc: false,
    jobUrl: undefined,
    ...overrides,
  };
}

// ── 1. Default tab selection ──────────────────────────────────────────────────

describe('JobAdView — default tab selection', () => {
  it('defaults to summary tab when description is present and not truncated', () => {
    render(<JobAdView {...makeProps()} />);
    // Summary content area is visible (no jobAdViewTextarea at initial render)
    // because we start on the summary tab. The textarea is behind the source tab.
    expect(screen.queryByTestId(TEST_IDS.documents.jobAdViewTextarea)).not.toBeInTheDocument();
    // The "Generate summary" button is rendered (summary tab empty state).
    expect(screen.getByText('autopilot.apply.jobAdView.generateSummary')).toBeInTheDocument();
  });

  it('defaults to source tab when hasDesc is false', () => {
    render(<JobAdView {...makeProps({ hasDesc: false, jobDesc: '' })} />);
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea)).toBeInTheDocument();
  });

  it('defaults to source tab when jobDesc ends with "…" (unicode ellipsis)', () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Some partial description…', hasDesc: true })} />);
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea)).toBeInTheDocument();
  });

  it('defaults to source tab when jobDesc ends with "..." (three dots)', () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Partial...', hasDesc: true })} />);
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea)).toBeInTheDocument();
  });

  it('defaults to summary tab when jobDesc ends with a normal character (not truncated)', () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Normal full description.', hasDesc: true })} />);
    expect(screen.queryByTestId(TEST_IDS.documents.jobAdViewTextarea)).not.toBeInTheDocument();
  });
});

// ── 2. TextArea always present on source tab ──────────────────────────────────

describe('JobAdView — TextArea always present on source tab', () => {
  async function switchToSource() {
    // Click the "Job ad" segmented control option (key passed through as-is by mock).
    await userEvent.click(screen.getByText('autopilot.apply.tabs.jobAd'));
  }

  it('shows an editable TextArea even when jobDesc is empty and hasDesc is false', async () => {
    render(<JobAdView {...makeProps({ hasDesc: false, jobDesc: '' })} />);
    // Already on source tab (default for no-desc).
    const textarea = screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);
    expect(textarea).toBeInTheDocument();
  });

  it('shows the paste placeholder when jobDesc is empty', () => {
    render(<JobAdView {...makeProps({ hasDesc: false, jobDesc: '' })} />);
    const textarea = screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);
    // Placeholder is set via the `placeholder` prop on TextArea → rendered on the underlying element.
    expect(textarea).toHaveAttribute('placeholder', 'autopilot.apply.jobAdView.pasteHint');
  });

  it('shows an editable TextArea on source tab even when starting on summary (normal desc)', async () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Normal full description.', hasDesc: true })} />);
    // Starts on summary — switch to source.
    await switchToSource();
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea)).toBeInTheDocument();
  });

  it('does NOT show the TextArea while fetchingDesc is true (loading state takes precedence)', async () => {
    render(<JobAdView {...makeProps({ hasDesc: false, jobDesc: '', fetchingDesc: true })} />);
    // The spinner/loading state replaces the textarea while fetching.
    expect(screen.queryByTestId(TEST_IDS.documents.jobAdViewTextarea)).not.toBeInTheDocument();
    expect(screen.getByText('autopilot.apply.fetchingDescription')).toBeInTheDocument();
  });
});

// ── 3. onJobDescChange fires on edit ─────────────────────────────────────────

describe('JobAdView — onJobDescChange callback', () => {
  it('calls onJobDescChange once per keystroke when the user types in the textarea', async () => {
    const onJobDescChange = vi.fn();
    render(<JobAdView {...makeProps({ hasDesc: false, jobDesc: '', onJobDescChange })} />);
    const textarea = screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);
    await userEvent.type(textarea, 'hello');
    // Controlled component fires one change event per character.
    expect(onJobDescChange).toHaveBeenCalledTimes(5);
    // Each call receives the current target value (a single char since the prop
    // doesn't update between renders in this controlled-stub setup).
    expect(onJobDescChange).toHaveBeenCalledWith(expect.any(String));
  });

  it('calls onJobDescChange when the user clears and re-types in the textarea', async () => {
    const onJobDescChange = vi.fn();
    render(<JobAdView {...makeProps({ hasDesc: true, jobDesc: 'Partial…', onJobDescChange })} />);
    // Truncated desc — already on source tab.
    const textarea = screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);
    await userEvent.clear(textarea);
    await userEvent.type(textarea, 'New text');
    expect(onJobDescChange).toHaveBeenCalled();
  });
});

// ── 4. Truncation hint ────────────────────────────────────────────────────────

describe('JobAdView — truncation hint', () => {
  it('shows the truncation hint Alert when jobDesc ends with unicode ellipsis', () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Short snippet…', hasDesc: true })} />);
    // The hint is now an Alert (role="alert") — auto-announced by screen readers.
    const alert = screen.getByRole('alert');
    expect(alert).toBeInTheDocument();
    expect(alert).toHaveTextContent('autopilot.apply.jobAdView.truncatedHint');
  });

  it('shows the truncation hint Alert when jobDesc ends with three dots', () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Short snippet...', hasDesc: true })} />);
    const alert = screen.getByRole('alert');
    expect(alert).toBeInTheDocument();
    expect(alert).toHaveTextContent('autopilot.apply.jobAdView.truncatedHint');
  });

  it('does NOT show the truncation hint for a normal (non-truncated) description', async () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Normal full description.', hasDesc: true })} />);
    // Switch to source tab to inspect.
    await userEvent.click(screen.getByText('autopilot.apply.tabs.jobAd'));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.apply.jobAdView.truncatedHint')).not.toBeInTheDocument();
  });

  it('does NOT show the truncation hint when jobDesc is empty (no text to hint about)', () => {
    render(<JobAdView {...makeProps({ hasDesc: false, jobDesc: '' })} />);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.apply.jobAdView.truncatedHint')).not.toBeInTheDocument();
  });
});

// ── 5. ExternalLink / viewJob ─────────────────────────────────────────────────

describe('JobAdView — view job link', () => {
  it('renders the "view job" link on source tab when jobUrl is provided', async () => {
    render(
      <JobAdView
        {...makeProps({ hasDesc: false, jobDesc: '', jobUrl: 'https://example.com/job' })}
      />
    );
    // Already on source tab (no desc).
    const link = screen.getByText('autopilot.viewJob').closest('a');
    expect(link).toHaveAttribute('href', 'https://example.com/job');
  });

  it('does NOT render the "view job" link when jobUrl is undefined', () => {
    render(<JobAdView {...makeProps({ hasDesc: false, jobDesc: '', jobUrl: undefined })} />);
    expect(screen.queryByText('autopilot.viewJob')).not.toBeInTheDocument();
  });

  it('renders the "view job" link even on the source tab when description is present', async () => {
    render(
      <JobAdView
        {...makeProps({
          jobDesc: 'Normal full description.',
          hasDesc: true,
          jobUrl: 'https://example.com/job',
        })}
      />
    );
    // Switch to source tab.
    await userEvent.click(screen.getByText('autopilot.apply.tabs.jobAd'));
    const link = screen.getByText('autopilot.viewJob').closest('a');
    expect(link).toHaveAttribute('href', 'https://example.com/job');
  });
});

// ── 6. TextArea a11y — aria-label + aria-describedby ─────────────────────────

describe('JobAdView — TextArea a11y wiring', () => {
  it('uses the short tab-label key as aria-label (NOT the full editHelper sentence)', () => {
    render(<JobAdView {...makeProps({ hasDesc: false, jobDesc: '' })} />);
    const textarea = screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);
    // aria-label must be the tab key (short name), not the full editHelper description
    expect(textarea).toHaveAttribute('aria-label', 'autopilot.apply.tabs.jobAd');
    expect(textarea).not.toHaveAttribute('aria-label', 'autopilot.apply.jobAdView.editHelper');
  });

  it('references the helper paragraph id via aria-describedby (non-truncated)', async () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Normal full description.', hasDesc: true })} />);
    await userEvent.click(screen.getByText('autopilot.apply.tabs.jobAd'));
    const textarea = screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);
    expect(textarea).toHaveAttribute('aria-describedby', 'job-ad-edit-helper');
    // The helper paragraph itself must carry the stable id
    const helperPara = document.getElementById('job-ad-edit-helper');
    expect(helperPara).toBeInTheDocument();
    expect(helperPara).toHaveTextContent('autopilot.apply.jobAdView.editHelper');
  });

  it('always uses only the helper id in aria-describedby (truncation hint is now an Alert, not an id-referenced element)', () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Short snippet…', hasDesc: true })} />);
    // Truncated → starts on source tab automatically
    const textarea = screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);
    // The Alert has role="alert" so screen readers auto-announce it;
    // aria-describedby only references the persistent helper paragraph.
    expect(textarea).toHaveAttribute('aria-describedby', 'job-ad-edit-helper');
    expect(textarea).not.toHaveAttribute(
      'aria-describedby',
      expect.stringContaining('job-ad-truncated-hint')
    );
    // The helper paragraph must carry its stable id
    expect(document.getElementById('job-ad-edit-helper')).toBeInTheDocument();
  });

  it('does NOT include truncation-hint id when description is not truncated', async () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Normal full description.', hasDesc: true })} />);
    await userEvent.click(screen.getByText('autopilot.apply.tabs.jobAd'));
    const textarea = screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);
    // Only helper id, no truncation-hint id
    expect(textarea).toHaveAttribute('aria-describedby', 'job-ad-edit-helper');
    expect(textarea).not.toHaveAttribute(
      'aria-describedby',
      expect.stringContaining('job-ad-truncated-hint')
    );
  });
});

// ── 7. ModelSelector renders on the summary tab ──────────────────────────────

describe('JobAdView — ModelSelector visibility', () => {
  it('renders ModelSelector when on the summary tab (default for full description)', () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Normal full description.', hasDesc: true })} />);
    // Default tab is summary for a non-truncated, present description.
    expect(screen.getByTestId('model-selector-stub')).toBeInTheDocument();
  });

  it('does NOT render ModelSelector when on the source tab', () => {
    // No description → defaults to source tab.
    render(<JobAdView {...makeProps({ hasDesc: false, jobDesc: '' })} />);
    expect(screen.queryByTestId('model-selector-stub')).not.toBeInTheDocument();
  });

  it('shows ModelSelector after switching to the summary tab', async () => {
    render(<JobAdView {...makeProps({ hasDesc: false, jobDesc: '' })} />);
    // Starts on source — ModelSelector not yet visible.
    expect(screen.queryByTestId('model-selector-stub')).not.toBeInTheDocument();
    // Switch to summary tab.
    await userEvent.click(screen.getByText('autopilot.apply.jobAdView.summaryTab'));
    expect(screen.getByTestId('model-selector-stub')).toBeInTheDocument();
  });

  it('hides ModelSelector after switching away from the summary tab', async () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Normal full description.', hasDesc: true })} />);
    // Starts on summary — ModelSelector visible.
    expect(screen.getByTestId('model-selector-stub')).toBeInTheDocument();
    // Switch to source tab.
    await userEvent.click(screen.getByText('autopilot.apply.tabs.jobAd'));
    expect(screen.queryByTestId('model-selector-stub')).not.toBeInTheDocument();
  });

  // Regression: the model dropdown + guidance line used to overflow the card's
  // right edge because `shrink-0` on ModelSelector pinned it to its full
  // intrinsic width while its wrapper row lacked `min-w-0`. jsdom can't measure
  // layout, so this asserts the structural fix (the classes that make it
  // shrink/truncate inside its row) rather than pixels.
  it('passes containment classes (min-w-0 flex-1) to ModelSelector so it shrinks inside the toolbar row', () => {
    render(<JobAdView {...makeProps({ jobDesc: 'Normal full description.', hasDesc: true })} />);
    const stub = screen.getByTestId('model-selector-stub');
    expect(stub.className).toContain('min-w-0');
    expect(stub.className).toContain('flex-1');
    expect(stub.className).not.toContain('shrink-0');
    // Its immediate row wrapper must also allow shrinking, or the fix on
    // ModelSelector alone can't stop the row itself from overflowing.
    expect(stub.parentElement?.className).toContain('min-w-0');
  });
});

// ── 8. Tab resync on posting change ──────────────────────────────────────────

describe('JobAdView — tab resync on posting change', () => {
  it('does NOT switch tab when jobDesc changes but jobUrl stays the same (no-yank guard)', () => {
    // Start on source tab (truncated posting).
    const { rerender } = render(
      <JobAdView
        {...makeProps({ jobUrl: 'https://example.com/job/1', jobDesc: 'Partial…', hasDesc: true })}
      />
    );
    // Confirm we started on source.
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea)).toBeInTheDocument();

    // Simulate the user pasting a full description — jobDesc changes, jobUrl stays the same.
    rerender(
      <JobAdView
        {...makeProps({
          jobUrl: 'https://example.com/job/1',
          jobDesc: 'Full description that is no longer truncated.',
          hasDesc: true,
        })}
      />
    );

    // Tab must NOT flip to summary — the user is still editing in the textarea.
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea)).toBeInTheDocument();
  });

  it('re-derives to summary when a new jobUrl arrives with a full description', () => {
    // Posting #1 — truncated, starts on source.
    const { rerender } = render(
      <JobAdView
        {...makeProps({ jobUrl: 'https://example.com/job/1', jobDesc: 'Partial…', hasDesc: true })}
      />
    );
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea)).toBeInTheDocument();

    // Navigate to posting #2 — full description, different URL.
    rerender(
      <JobAdView
        {...makeProps({
          jobUrl: 'https://example.com/job/2',
          jobDesc: 'Full description with plenty of content.',
          hasDesc: true,
        })}
      />
    );

    // Tab should re-derive to summary (full desc, not truncated).
    expect(screen.queryByTestId(TEST_IDS.documents.jobAdViewTextarea)).not.toBeInTheDocument();
    expect(screen.getByText('autopilot.apply.jobAdView.generateSummary')).toBeInTheDocument();
  });

  it('re-derives to source when a new jobUrl arrives with no description (hasDesc false)', () => {
    // Posting #1 — full description, starts on summary.
    const { rerender } = render(
      <JobAdView
        {...makeProps({
          jobUrl: 'https://example.com/job/1',
          jobDesc: 'Full description with plenty of content.',
          hasDesc: true,
        })}
      />
    );
    expect(screen.queryByTestId(TEST_IDS.documents.jobAdViewTextarea)).not.toBeInTheDocument();

    // Navigate to posting #2 — no description.
    rerender(
      <JobAdView
        {...makeProps({
          jobUrl: 'https://example.com/job/2',
          jobDesc: '',
          hasDesc: false,
        })}
      />
    );

    // Tab should re-derive to source (no desc).
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea)).toBeInTheDocument();
  });
});
