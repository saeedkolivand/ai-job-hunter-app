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
 *   7. ModelSelector visibility on the summary tab.
 *   8. Tab resync on posting change.
 *
 * Score-tab wiring + query gating live in `JobAdView/scoreTab.test.tsx`; shared stubs in
 * `JobAdView/test-support.tsx`. `@ajh/translations` returns keys as-is (deterministic assertions).
 * noUncheckedIndexedAccess: all array index accesses are guarded.
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { JobAdView } from './JobAdView';
import { makeProps } from './JobAdView/test-support';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/components/ui/ModelSelector', async () => {
  return (await import('./JobAdView/test-support')).modelSelectorModule;
});
vi.mock('@/components/ui/ExternalLink', async () => {
  return (await import('./JobAdView/test-support')).externalLinkModule;
});
vi.mock('@/lib/generate', async () => (await import('./JobAdView/test-support')).generateModule);
vi.mock('@/services', async () => (await import('./JobAdView/test-support')).servicesModule);

type Overrides = Parameters<typeof makeProps>[0];
const renderView = (overrides: Overrides = {}) => render(<JobAdView {...makeProps(overrides)} />);
const rerenderView = (r: ReturnType<typeof renderView>, overrides: Overrides) =>
  r.rerender(<JobAdView {...makeProps(overrides)} />);
const textarea = () => screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);
const queryTextarea = () => screen.queryByTestId(TEST_IDS.documents.jobAdViewTextarea);
const clickTab = (label: string) => userEvent.click(screen.getByText(`autopilot.apply.${label}`));
/** No description at all — the view starts on the source tab. */
const EMPTY: Overrides = { hasDesc: false, jobDesc: '' };
const FULL: Overrides = { jobDesc: 'Normal full description.', hasDesc: true };

// ── 1. Default tab selection ──────────────────────────────────────────────────

describe('JobAdView — default tab selection', () => {
  it('defaults to summary tab when description is present and not truncated', () => {
    renderView();
    // Summary content area is visible (no jobAdViewTextarea at initial render)
    // because we start on the summary tab. The textarea is behind the source tab.
    expect(queryTextarea()).not.toBeInTheDocument();
    // The "Generate summary" button is rendered (summary tab empty state).
    expect(screen.getByText('autopilot.apply.jobAdView.generateSummary')).toBeInTheDocument();
  });

  it.each([
    ['defaults to source tab when hasDesc is false', EMPTY],
    [
      'defaults to source tab when jobDesc ends with "…" (unicode ellipsis)',
      { jobDesc: 'Some partial description…', hasDesc: true },
    ],
    [
      'defaults to source tab when jobDesc ends with "..." (three dots)',
      { jobDesc: 'Partial...', hasDesc: true },
    ],
  ])('%s', (_name, overrides) => {
    renderView(overrides);
    expect(textarea()).toBeInTheDocument();
  });

  it('defaults to summary tab when jobDesc ends with a normal character (not truncated)', () => {
    renderView(FULL);
    expect(queryTextarea()).not.toBeInTheDocument();
  });
});

// ── 2. TextArea always present on source tab ──────────────────────────────────

describe('JobAdView — TextArea always present on source tab', () => {
  it('shows an editable TextArea even when jobDesc is empty and hasDesc is false', () => {
    renderView(EMPTY);
    // Already on source tab (default for no-desc).
    expect(textarea()).toBeInTheDocument();
  });

  it('shows the paste placeholder when jobDesc is empty', () => {
    renderView(EMPTY);
    // Placeholder is set via the `placeholder` prop on TextArea → rendered on the underlying element.
    expect(textarea()).toHaveAttribute('placeholder', 'autopilot.apply.jobAdView.pasteHint');
  });

  it('shows an editable TextArea on source tab even when starting on summary (normal desc)', async () => {
    renderView(FULL);
    // Starts on summary — switch to source.
    await clickTab('tabs.jobAd');
    expect(textarea()).toBeInTheDocument();
  });

  it('does NOT show the TextArea while fetchingDesc is true (loading state takes precedence)', () => {
    renderView({ ...EMPTY, fetchingDesc: true });
    // The spinner/loading state replaces the textarea while fetching.
    expect(queryTextarea()).not.toBeInTheDocument();
    expect(screen.getByText('autopilot.apply.fetchingDescription')).toBeInTheDocument();
  });
});

// ── 3. onJobDescChange fires on edit ─────────────────────────────────────────

describe('JobAdView — onJobDescChange callback', () => {
  it('calls onJobDescChange once per keystroke when the user types in the textarea', async () => {
    const onJobDescChange = vi.fn();
    renderView({ ...EMPTY, onJobDescChange });
    await userEvent.type(textarea(), 'hello');
    // Controlled component fires one change event per character.
    expect(onJobDescChange).toHaveBeenCalledTimes(5);
    // Each call receives the current target value (a single char since the prop
    // doesn't update between renders in this controlled-stub setup).
    expect(onJobDescChange).toHaveBeenCalledWith(expect.any(String));
  });

  it('calls onJobDescChange when the user clears and re-types in the textarea', async () => {
    const onJobDescChange = vi.fn();
    renderView({ hasDesc: true, jobDesc: 'Partial…', onJobDescChange });
    // Truncated desc — already on source tab.
    await userEvent.clear(textarea());
    await userEvent.type(textarea(), 'New text');
    expect(onJobDescChange).toHaveBeenCalled();
  });
});

// ── 4. Truncation hint ────────────────────────────────────────────────────────

describe('JobAdView — truncation hint', () => {
  it.each([
    ['unicode ellipsis', 'Short snippet…'],
    ['three dots', 'Short snippet...'],
  ])('shows the truncation hint Alert when jobDesc ends with %s', (_name, jobDesc) => {
    renderView({ jobDesc, hasDesc: true });
    // The hint is now an Alert (role="alert") — auto-announced by screen readers.
    const alert = screen.getByRole('alert');
    expect(alert).toBeInTheDocument();
    expect(alert).toHaveTextContent('autopilot.apply.jobAdView.truncatedHint');
  });

  it.each([
    ['for a normal (non-truncated) description', FULL, true],
    ['when jobDesc is empty (no text to hint about)', EMPTY, false],
  ])('does NOT show the truncation hint %s', async (_name, overrides, switchToSource) => {
    renderView(overrides);
    // Switch to source tab to inspect.
    if (switchToSource) await clickTab('tabs.jobAd');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.queryByText('autopilot.apply.jobAdView.truncatedHint')).not.toBeInTheDocument();
  });
});

// ── 5. ExternalLink / viewJob ─────────────────────────────────────────────────

describe('JobAdView — view job link', () => {
  const jobUrl = 'https://example.com/job';

  it.each([
    ['renders the "view job" link on source tab when jobUrl is provided', EMPTY, false],
    ['renders the "view job" link even on the source tab when description is present', FULL, true],
  ])('%s', async (_name, overrides, switchToSource) => {
    renderView({ ...overrides, jobUrl });
    if (switchToSource) await clickTab('tabs.jobAd');
    expect(screen.getByText('autopilot.viewJob').closest('a')).toHaveAttribute('href', jobUrl);
  });

  it('does NOT render the "view job" link when jobUrl is undefined', () => {
    renderView({ ...EMPTY, jobUrl: undefined });
    expect(screen.queryByText('autopilot.viewJob')).not.toBeInTheDocument();
  });
});

// ── 6. TextArea a11y — aria-label + aria-describedby ─────────────────────────

describe('JobAdView — TextArea a11y wiring', () => {
  it('uses the short tab-label key as aria-label (NOT the full editHelper sentence)', () => {
    renderView(EMPTY);
    // aria-label must be the tab key (short name), not the full editHelper description
    expect(textarea()).toHaveAttribute('aria-label', 'autopilot.apply.tabs.jobAd');
    expect(textarea()).not.toHaveAttribute('aria-label', 'autopilot.apply.jobAdView.editHelper');
  });

  it('references the helper paragraph id via aria-describedby (non-truncated)', async () => {
    renderView(FULL);
    await clickTab('tabs.jobAd');
    expect(textarea()).toHaveAttribute('aria-describedby', 'job-ad-edit-helper');
    // The helper paragraph itself must carry the stable id
    const helperPara = document.getElementById('job-ad-edit-helper');
    expect(helperPara).toBeInTheDocument();
    expect(helperPara).toHaveTextContent('autopilot.apply.jobAdView.editHelper');
  });

  // Truncated → starts on source tab automatically. The Alert has role="alert" so
  // screen readers auto-announce it; aria-describedby only references the
  // persistent helper paragraph (the truncation hint is no longer id-referenced).
  it.each([
    [
      'always uses only the helper id in aria-describedby (truncation hint is now an Alert, not an id-referenced element)',
      { jobDesc: 'Short snippet…', hasDesc: true },
      false,
    ],
    ['does NOT include truncation-hint id when description is not truncated', FULL, true],
  ])('%s', async (_name, overrides, switchToSource) => {
    renderView(overrides);
    if (switchToSource) await clickTab('tabs.jobAd');
    expect(textarea()).toHaveAttribute('aria-describedby', 'job-ad-edit-helper');
    expect(textarea()).not.toHaveAttribute(
      'aria-describedby',
      expect.stringContaining('job-ad-truncated-hint')
    );
    // The helper paragraph must carry its stable id
    if (!switchToSource) expect(document.getElementById('job-ad-edit-helper')).toBeInTheDocument();
  });
});

// ── 7. ModelSelector renders on the summary tab ──────────────────────────────

describe('JobAdView — ModelSelector visibility', () => {
  const selector = () => screen.queryByTestId('model-selector-stub');

  it('renders ModelSelector when on the summary tab (default for full description)', () => {
    renderView(FULL);
    // Default tab is summary for a non-truncated, present description.
    expect(selector()).toBeInTheDocument();
  });

  it('does NOT render ModelSelector when on the source tab', () => {
    // No description → defaults to source tab.
    renderView(EMPTY);
    expect(selector()).not.toBeInTheDocument();
  });

  it('shows ModelSelector after switching to the summary tab', async () => {
    renderView(EMPTY);
    // Starts on source — ModelSelector not yet visible.
    expect(selector()).not.toBeInTheDocument();
    // Switch to summary tab.
    await clickTab('jobAdView.summaryTab');
    expect(selector()).toBeInTheDocument();
  });

  it('hides ModelSelector after switching away from the summary tab', async () => {
    renderView(FULL);
    // Starts on summary — ModelSelector visible.
    expect(selector()).toBeInTheDocument();
    // Switch to source tab.
    await clickTab('tabs.jobAd');
    expect(selector()).not.toBeInTheDocument();
  });

  // Regression: the model dropdown + guidance line used to overflow the card's
  // right edge because `shrink-0` on ModelSelector pinned it to its full
  // intrinsic width while its wrapper row lacked `min-w-0`. jsdom can't measure
  // layout, so this asserts the structural fix (the classes that make it
  // shrink/truncate inside its row) rather than pixels.
  it('passes the containment class (min-w-0) to ModelSelector so it shrinks inside the toolbar row, without stretching it', () => {
    renderView(FULL);
    const stub = screen.getByTestId('model-selector-stub');
    expect(stub).toHaveClass('min-w-0');
    expect(stub).not.toHaveClass('shrink-0');
    expect(stub).not.toHaveClass('flex-1');
    // Its immediate row wrapper must also allow shrinking, or the fix on
    // ModelSelector alone can't stop the row itself from overflowing.
    expect(stub.parentElement).toHaveClass('min-w-0');
  });
});

// ── 8. Tab resync on posting change ──────────────────────────────────────────

describe('JobAdView — tab resync on posting change', () => {
  const job = (n: number, overrides: Overrides) => ({
    jobUrl: `https://example.com/job/${n}`,
    ...overrides,
  });

  it('does NOT switch tab when jobDesc changes but jobUrl stays the same (no-yank guard)', () => {
    // Start on source tab (truncated posting).
    const view = renderView(job(1, { jobDesc: 'Partial…', hasDesc: true }));
    // Confirm we started on source.
    expect(textarea()).toBeInTheDocument();

    // Simulate the user pasting a full description — jobDesc changes, jobUrl stays the same.
    rerenderView(
      view,
      job(1, { jobDesc: 'Full description that is no longer truncated.', hasDesc: true })
    );

    // Tab must NOT flip to summary — the user is still editing in the textarea.
    expect(textarea()).toBeInTheDocument();
  });

  it('re-derives to summary when a new jobUrl arrives with a full description', () => {
    // Posting #1 — truncated, starts on source.
    const view = renderView(job(1, { jobDesc: 'Partial…', hasDesc: true }));
    expect(textarea()).toBeInTheDocument();

    // Navigate to posting #2 — full description, different URL.
    rerenderView(
      view,
      job(2, { jobDesc: 'Full description with plenty of content.', hasDesc: true })
    );

    // Tab should re-derive to summary (full desc, not truncated).
    expect(queryTextarea()).not.toBeInTheDocument();
    expect(screen.getByText('autopilot.apply.jobAdView.generateSummary')).toBeInTheDocument();
  });

  it('re-derives to source when a new jobUrl arrives with no description (hasDesc false)', () => {
    // Posting #1 — full description, starts on summary.
    const view = renderView(
      job(1, { jobDesc: 'Full description with plenty of content.', hasDesc: true })
    );
    expect(queryTextarea()).not.toBeInTheDocument();

    // Navigate to posting #2 — no description.
    rerenderView(view, job(2, EMPTY));

    // Tab should re-derive to source (no desc).
    expect(textarea()).toBeInTheDocument();
  });
});
