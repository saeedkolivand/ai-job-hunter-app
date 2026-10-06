/**
 * GenerationOutput — tabs wiring, aria-selected and tabpanel ARIA linkage.
 * Mocks, props builder and helpers live in `harness.tsx` (see its header).
 */
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import {
  clickJobAdTab,
  clickSourceSubTab,
  GenerationOutput,
  makeProps,
  renderOutput,
  resetHarness,
} from './GenerationOutput/harness';

beforeEach(resetHarness);

const tab = (name: string) => screen.getByRole('tab', { name: `autopilot.apply.${name}` });
const queryTab = (name: string) => screen.queryByRole('tab', { name: `autopilot.apply.${name}` });
const expectSelected = (name: string, selected: boolean) =>
  expect(tab(name)).toHaveAttribute('aria-selected', String(selected));

describe('GenerationOutput', () => {
  // ── 1. Job ad tab shows jobDesc read-only ────────────────────────────────────

  describe('Job ad tab', () => {
    it('shows the jobDesc text in an editable TextArea after clicking the Job ad tab then the source sub-tab', async () => {
      const user = userEvent.setup();
      renderOutput();

      await clickJobAdTab(user);
      // JobAdView defaults to the Summary sub-tab — switch to the source sub-tab.
      await clickSourceSubTab(user);

      // The job description text must be visible in the editable TextArea.
      expect(screen.getByDisplayValue('Full job description text')).toBeInTheDocument();

      // The doc EditableOutput must NOT be mounted while the Job ad tab is active.
      expect(screen.queryByTestId(TEST_IDS.documents.editableOutput)).not.toBeInTheDocument();
    });

    it('shows the summary empty-state Generate button and calls generate on click', async () => {
      const user = userEvent.setup();
      const generate = vi.fn();
      renderOutput({
        jobAdSummary: {
          summary: '',
          generating: false,
          error: null,
          generate,
          language: 'en',
          setLanguage: vi.fn(),
        },
      });

      await clickJobAdTab(user);

      const generateBtn = screen.getByRole('button', {
        name: /autopilot\.apply\.jobAdView\.generateSummary/i,
      });
      expect(generateBtn).toBeInTheDocument();

      await user.click(generateBtn);
      expect(generate).toHaveBeenCalledTimes(1);
    });

    it('selecting a summary language calls setLanguage with the locale code', async () => {
      const user = userEvent.setup();
      const setLanguage = vi.fn();
      renderOutput({
        jobAdSummary: {
          summary: '',
          generating: false,
          error: null,
          generate: vi.fn(),
          language: 'en',
          setLanguage,
        },
      });

      await clickJobAdTab(user);

      // The picker carries an explicit label binding (sr-only <label htmlFor>).
      expect(screen.getByText('autopilot.apply.jobAdView.summaryLanguage')).toHaveAttribute(
        'for',
        'job-ad-summary-language'
      );

      // Summary sub-tab is the default; the language picker lists OUTPUT_LANGUAGES
      // by endonym. Choosing German must forward its locale CODE ('de'), not the
      // display name (which safeLocale would collapse to English).
      await user.click(screen.getByRole('option', { name: 'Deutsch' }));

      expect(setLanguage).toHaveBeenCalledWith('de');
    });

    it('hides the editable doc output while Job ad tab is active', async () => {
      const user = userEvent.setup();
      renderOutput();

      // EditableOutput is present initially (doc view).
      expect(screen.getByTestId(TEST_IDS.documents.editableOutput)).toBeInTheDocument();

      await clickJobAdTab(user);

      // EditableOutput must be gone after switching to job-ad view.
      expect(screen.queryByTestId(TEST_IDS.documents.editableOutput)).not.toBeInTheDocument();
    });
  });

  // ── 2./3. Copy + Export are disabled on the Job ad tab ───────────────────────

  describe.each([
    ['Copy button', /autopilot\.apply\.copy/i],
    ['Export button', /aiGenerate\.export/i],
  ])('%s', (_label, name) => {
    it('is enabled on the doc tab when output is non-empty', () => {
      renderOutput();
      expect(screen.getByRole('button', { name })).not.toBeDisabled();
    });

    it('is disabled after switching to the Job ad tab', async () => {
      const user = userEvent.setup();
      renderOutput();

      await clickJobAdTab(user);

      expect(screen.getByRole('button', { name })).toBeDisabled();
    });
  });

  // ── 4. Doc tab drives setActiveOut ───────────────────────────────────────────

  describe('Doc tab wiring', () => {
    it.each([
      ['cover', 'resume'],
      ['resume', 'cover'],
    ] as const)(
      'calls setActiveOut("%s") when its tab is clicked from the %s tab (target="both")',
      async (clicked, active) => {
        const user = userEvent.setup();
        const setActiveOut = vi.fn();
        renderOutput({ target: 'both', activeOut: active, setActiveOut });

        await user.click(tab(`target.${clicked}`));

        expect(setActiveOut).toHaveBeenCalledTimes(1);
        expect(setActiveOut).toHaveBeenCalledWith(clicked);
      }
    );

    it('does not render the Cover tab when target="resume"', () => {
      renderOutput({ target: 'resume', activeOut: 'resume' });
      expect(queryTab('target.cover')).not.toBeInTheDocument();
    });

    // The reported bug's visible half. The tab LIST used to be built from
    // `activeOut` (`target === 'both' ? [...] : [activeOut]`), which for a
    // cover-only run — whose `activeOut` was an uncorrected `'resume'` — put a
    // single tab on screen labelled "Resume", showing the résumé. Passed the
    // INCONSISTENT prop pair the old code actually produced, so it fails
    // against a build that reads `activeOut` again.
    it('does not render the Resume tab for a cover-only run with nothing saved', () => {
      renderOutput({ target: 'cover', activeOut: 'resume', hasResume: false });
      expect(queryTab('target.resume')).not.toBeInTheDocument();
      // …and the letter's own tab IS there — otherwise this would pass just as
      // happily against a build that rendered no document tabs at all.
      expect(tab('target.cover')).toBeInTheDocument();
    });

    it('keeps the Resume tab for a cover-only run on a posting that already has one', () => {
      renderOutput({ target: 'cover', activeOut: 'cover', hasResume: true });
      expect(tab('target.cover')).toBeInTheDocument();
      // The saved document from an earlier run stays viewable and exportable.
      expect(tab('target.resume')).toBeInTheDocument();
    });
  });

  // ── 5. aria-selected reflects active tab (tab pattern) ───────────────────────

  describe('aria-selected state', () => {
    it('tabs are grouped in a tablist', () => {
      renderOutput({ target: 'both', activeOut: 'resume' });
      expect(screen.getByRole('tablist')).toBeInTheDocument();
    });

    it('active doc tab has aria-selected="true", inactive tabs have aria-selected="false"', () => {
      renderOutput({ target: 'both', activeOut: 'resume' });

      // Resume tab is active — aria-selected must be true.
      expectSelected('target.resume', true);
      // Cover tab and Job ad tab are inactive.
      expectSelected('target.cover', false);
      expectSelected('tabs.jobAd', false);
    });

    it('Job ad tab has aria-selected="true" after being clicked', async () => {
      const user = userEvent.setup();
      renderOutput({ target: 'both', activeOut: 'resume' });

      await clickJobAdTab(user);

      expectSelected('tabs.jobAd', true);
      // Doc tabs must now be unselected.
      expectSelected('target.resume', false);
    });

    it('switching back to a doc tab sets its aria-selected="true" and Job ad tab to "false"', async () => {
      const user = userEvent.setup();
      renderOutput({ target: 'both', activeOut: 'resume' });

      // Switch to job ad view, then back to the resume tab.
      await clickJobAdTab(user);
      await user.click(tab('target.resume'));

      expectSelected('target.resume', true);
      expectSelected('tabs.jobAd', false);
    });
  });

  // ── 9. Tabpanel ARIA linkage ──────────────────────────────────────────────
  // The single `role="tabpanel"` region must carry `id`, `aria-labelledby`,
  // and `aria-controls` wired to the ACTIVE tab. The active tab must carry a
  // matching `aria-controls` pointing to the panel id.

  describe('Tabpanel ARIA linkage', () => {
    it('tabpanel has role="tabpanel" with a non-empty id', () => {
      renderOutput({ target: 'both', activeOut: 'resume' });
      const panel = screen.getByRole('tabpanel');
      expect(panel).toBeInTheDocument();
      expect(panel.id).toBeTruthy();
    });

    it.each(['resume', 'cover'] as const)(
      'tabpanel id is "tailor-panel-%s" when that tab is active',
      (activeOut) => {
        renderOutput({ target: 'both', activeOut });
        expect(screen.getByRole('tabpanel').id).toBe(`tailor-panel-${activeOut}`);
      }
    );

    it('tabpanel id is "tailor-panel-jobad" when the Job ad tab is active', async () => {
      const user = userEvent.setup();
      renderOutput({ target: 'both', activeOut: 'resume' });

      await clickJobAdTab(user);

      expect(screen.getByRole('tabpanel').id).toBe('tailor-panel-jobad');
    });

    it('tabpanel aria-labelledby matches the id of the active tab', () => {
      renderOutput({ target: 'both', activeOut: 'resume' });
      const panel = screen.getByRole('tabpanel');
      const labelledBy = panel.getAttribute('aria-labelledby');
      expect(labelledBy).toBe('tailor-tab-resume');
      // The tab element with that id must exist.
      expect(document.getElementById('tailor-tab-resume')).toBeInTheDocument();
    });

    it.each([
      ['resume', 'target.resume', 'resume'],
      ['cover', 'target.cover', 'cover'],
      ['resume', 'tabs.jobAd', 'jobad'],
    ] as const)(
      'with the %s tab active, the %s tab aria-controls points to "tailor-panel-%s"',
      (activeOut, name, panel) => {
        renderOutput({ target: 'both', activeOut });
        expect(tab(name).getAttribute('aria-controls')).toBe(`tailor-panel-${panel}`);
      }
    );

    it('tabpanel aria-labelledby updates to the job ad tab id after switching to job ad view', async () => {
      const user = userEvent.setup();
      renderOutput({ target: 'both', activeOut: 'resume' });

      await clickJobAdTab(user);

      const panel = screen.getByRole('tabpanel');
      expect(panel.getAttribute('aria-labelledby')).toBe('tailor-tab-jobad');
    });

    it('tabpanel aria-labelledby updates when switching from resume to cover tab', async () => {
      const user = userEvent.setup();
      // doc-tab switches update `activeOut` via setActiveOut — needs a stateful
      // wrapper that mirrors the parent's controlled-prop round-trip.
      function ActiveOutWrapper() {
        const [activeOut, setActiveOut] = React.useState<'resume' | 'cover'>('resume');
        return <GenerationOutput {...makeProps({ target: 'both', activeOut, setActiveOut })} />;
      }
      render(<ActiveOutWrapper />);

      await user.click(tab('target.cover'));

      const panel = screen.getByRole('tabpanel');
      expect(panel.getAttribute('aria-labelledby')).toBe('tailor-tab-cover');
      expect(panel.id).toBe('tailor-panel-cover');
    });

    it('tabpanel has tabIndex={0} for keyboard reachability', () => {
      renderOutput({ target: 'both', activeOut: 'resume' });
      expect(screen.getByRole('tabpanel')).toHaveAttribute('tabindex', '0');
    });
  });
});
