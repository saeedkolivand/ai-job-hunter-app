import React, { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { MatchScore } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';

import { hashText, type QualityReport, TEMPLATES } from '@/lib/generate';

import { GenerationOutput } from './GenerationOutput';

// ── Module stubs ──────────────────────────────────────────────────────────────

// Echo every key verbatim — no i18next runtime needed in jsdom.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ModelSelector uses useAppClient (requires AppClientProvider) — stub so tests
// that reach the summary tab don't need a full provider tree. `useSelectedProvider`
// reads a mutable module-level var (not a plain arrow) so the CLI-agent egress
// tests below can select a CLI-agent provider; every other test relies on the
// default ('ollama' — not a CLI agent, so the score strip never discloses egress).
let mockActiveProvider = 'ollama';

vi.mock('@/components/ui/ModelSelector', () => ({
  ModelSelector: () => <div data-testid="model-selector-stub" />,
  useSelectedProvider: () => mockActiveProvider,
}));

// ExternalLink uses useAppClient (requires AppClientProvider) — stub it with a
// plain anchor so tests that reach the Job-ad source tab don't need a provider.
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

// useJobAdTextMatchScore — shared by JobAdView's Score tab AND the résumé
// result's score strip (GenerationScoreStrip), via useAppClient/QueryClient —
// stubbed so tests that reach either don't need a provider tree. A mutable
// `stubbedScore` (not a plain arrow) so the score-strip tests below can drive
// it — same pattern as JobAdView.i18n.test.tsx. `mockUseJobAdTextMatchScore`
// (the `mock`-prefixed name) is Vitest's documented exception to the "no
// out-of-scope refs in a hoisted factory" rule. Reset before EVERY test —
// most never touch it and rely on this default (undefined data, not
// loading), which is what makes the strip render its honest "not scored"
// placeholder rather than a stale value leaking across tests.
let stubbedScore: {
  data?: unknown;
  isLoading?: boolean;
  isError?: boolean;
  refetch?: () => void;
} = { data: undefined, isLoading: false };

beforeEach(() => {
  stubbedScore = { data: undefined, isLoading: false };
  mockActiveProvider = 'ollama';
});

const mockUseJobAdTextMatchScore = vi.fn((..._args: unknown[]) => stubbedScore);

vi.mock('@/services', () => ({
  useJobAdTextMatchScore: (...args: unknown[]) => mockUseJobAdTextMatchScore(...args),
}));

// EditableOutput mock — exposes onChange/onBlur/isPending + renders previewSlot.
// Uses divs (not raw <textarea>/<button>) to stay clear of the @ajh/ui ESLint rule.
// The mock is intentionally richer than the original so edit/debounce/preview tests
// can drive the component's committed-text logic without the real editor tree.
vi.mock('@/components/generation/EditableOutput', () => ({
  EditableOutput: ({
    value,
    onChange,
    onBlur,
    isPending,
    previewSlot,
  }: {
    value: string;
    onChange?: (v: string) => void;
    onBlur?: () => void;
    isPending?: boolean;
    previewSlot?: React.ReactNode;
  }) => (
    <div data-testid={TEST_IDS.documents.editableOutput}>
      {value}
      <div
        role="textbox"
        data-testid={TEST_IDS.documents.editableInput}
        contentEditable
        suppressContentEditableWarning
        onInput={(e) => onChange?.((e.target as HTMLElement).textContent ?? '')}
        onBlur={onBlur}
      />
      {isPending && <div data-testid={TEST_IDS.generation.pendingCommit}>updating</div>}
      {previewSlot && <div data-testid={TEST_IDS.documents.previewSlot}>{previewSlot}</div>}
    </div>
  ),
}));

// PdfPreview mock — renders its `text` and `locale` props into a testid (the
// latter as a data attribute) so tests can inspect the committed text AND the
// market/locale GenerationOutput actually forwards, without launching the real
// Typst/PDF pipeline.
vi.mock('@/components/generation/PdfPreview', () => ({
  PdfPreview: ({ text, locale }: { text: string; locale?: string }) => (
    <div data-testid={TEST_IDS.documents.pdfPreview} data-locale={locale ?? ''}>
      {text}
    </div>
  ),
}));

// Dropdown mock — preserves all other @ajh/ui exports unchanged via
// importOriginal; only Dropdown is replaced with a plain <select>-like
// div structure that drives onChange when an option div is clicked.
vi.mock('@ajh/ui', async (importOriginal) => {
  const real = await importOriginal<typeof AjhUi>();
  return {
    ...real,
    Dropdown: ({
      options,
      value,
      onChange,
      id,
    }: {
      options: Array<{ value: string; label: string }>;
      value: string;
      onChange: (v: string) => void;
      id?: string;
    }) => (
      <div data-testid={id ?? 'dropdown'} data-value={value}>
        {options.map((o) => (
          <div
            key={o.value}
            role="option"
            aria-selected={o.value === value}
            data-optvalue={o.value}
            onClick={() => onChange(o.value)}
          >
            {o.label}
          </div>
        ))}
      </div>
    ),
  };
});

// ── Default props fixture ─────────────────────────────────────────────────────

const noop = () => undefined;

function makeProps(overrides: Partial<Parameters<typeof GenerationOutput>[0]> = {}) {
  return {
    target: 'both' as const,
    activeOut: 'resume' as const,
    setActiveOut: vi.fn(),
    templateId: 'classic' as const,
    atsMode: false,
    accent: undefined,
    letterLayoutId: undefined,
    onTemplateChange: vi.fn(),
    onAtsModeChange: vi.fn(),
    onAccentChange: vi.fn(),
    onLetterLayoutChange: vi.fn(),
    output: 'Generated resume content',
    onEdit: noop,
    editable: false,
    meta: null,
    copied: false,
    onCopy: noop,
    exportOpen: false,
    setExportOpen: vi.fn(),
    onExport: vi.fn(),
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
    ...overrides,
  };
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Click the top-level "Job ad" tab (its label is the echoed i18n key). */
async function clickJobAdTab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('tab', { name: 'autopilot.apply.tabs.jobAd' }));
}

/** Click the JobAdView "Job ad" source SUB-TAB (a SegmentedControl radio). */
async function clickSourceSubTab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('radio', { name: 'autopilot.apply.tabs.jobAd' }));
}

/** Click a template option by its id inside the picker. */
async function pickTemplate(user: ReturnType<typeof userEvent.setup>, templateId: string) {
  await user.click(screen.getByRole('option', { name: new RegExp(templateId, 'i') }));
}

// ── Stateful wrapper for edit/save/preview tests ───────────────────────────────
// The component is fully controlled: onEdit informs the parent, the parent must
// pass the new value back down as `output`. This wrapper simulates that round-trip.

function ControlledWrapper(initialProps: Parameters<typeof GenerationOutput>[0]) {
  const [output, setOutput] = useState(initialProps.output);
  const handleEdit = (text: string) => {
    initialProps.onEdit(text);
    setOutput(text);
  };
  return <GenerationOutput {...initialProps} output={output} onEdit={handleEdit} />;
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('GenerationOutput', () => {
  // ── 1. Job ad tab shows jobDesc read-only ────────────────────────────────────

  describe('Job ad tab', () => {
    it('shows the jobDesc text in an editable TextArea after clicking the Job ad tab then the source sub-tab', async () => {
      const user = userEvent.setup();
      render(<GenerationOutput {...makeProps()} />);

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
      render(
        <GenerationOutput
          {...makeProps({
            jobAdSummary: {
              summary: '',
              generating: false,
              error: null,
              generate,
              language: 'en',
              setLanguage: vi.fn(),
            },
          })}
        />
      );

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
      render(
        <GenerationOutput
          {...makeProps({
            jobAdSummary: {
              summary: '',
              generating: false,
              error: null,
              generate: vi.fn(),
              language: 'en',
              setLanguage,
            },
          })}
        />
      );

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
      render(<GenerationOutput {...makeProps()} />);

      // EditableOutput is present initially (doc view).
      expect(screen.getByTestId(TEST_IDS.documents.editableOutput)).toBeInTheDocument();

      await clickJobAdTab(user);

      // EditableOutput must be gone after switching to job-ad view.
      expect(screen.queryByTestId(TEST_IDS.documents.editableOutput)).not.toBeInTheDocument();
    });
  });

  // ── 2. Copy disabled on Job ad tab ───────────────────────────────────────────

  describe('Copy button', () => {
    it('is enabled on the doc tab when output is non-empty', () => {
      render(<GenerationOutput {...makeProps()} />);
      const copyBtn = screen.getByRole('button', { name: /autopilot\.apply\.copy/i });
      expect(copyBtn).not.toBeDisabled();
    });

    it('is disabled after switching to the Job ad tab', async () => {
      const user = userEvent.setup();
      render(<GenerationOutput {...makeProps()} />);

      await clickJobAdTab(user);

      const copyBtn = screen.getByRole('button', { name: /autopilot\.apply\.copy/i });
      expect(copyBtn).toBeDisabled();
    });
  });

  // ── 3. Export disabled on Job ad tab ─────────────────────────────────────────

  describe('Export button', () => {
    it('is enabled on the doc tab when output is non-empty', () => {
      render(<GenerationOutput {...makeProps()} />);
      const exportBtn = screen.getByRole('button', { name: /aiGenerate\.export/i });
      expect(exportBtn).not.toBeDisabled();
    });

    it('is disabled after switching to the Job ad tab', async () => {
      const user = userEvent.setup();
      render(<GenerationOutput {...makeProps()} />);

      await clickJobAdTab(user);

      const exportBtn = screen.getByRole('button', { name: /aiGenerate\.export/i });
      expect(exportBtn).toBeDisabled();
    });
  });

  // ── 4. Doc tab drives setActiveOut ───────────────────────────────────────────

  describe('Doc tab wiring', () => {
    it('calls setActiveOut("cover") when the Cover tab is clicked (target="both")', async () => {
      const user = userEvent.setup();
      const setActiveOut = vi.fn();
      render(
        <GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume', setActiveOut })} />
      );

      await user.click(screen.getByRole('tab', { name: 'autopilot.apply.target.cover' }));

      expect(setActiveOut).toHaveBeenCalledTimes(1);
      expect(setActiveOut).toHaveBeenCalledWith('cover');
    });

    it('calls setActiveOut("resume") when the Resume tab is clicked (target="both")', async () => {
      const user = userEvent.setup();
      const setActiveOut = vi.fn();
      render(
        <GenerationOutput {...makeProps({ target: 'both', activeOut: 'cover', setActiveOut })} />
      );

      await user.click(screen.getByRole('tab', { name: 'autopilot.apply.target.resume' }));

      expect(setActiveOut).toHaveBeenCalledTimes(1);
      expect(setActiveOut).toHaveBeenCalledWith('resume');
    });

    it('does not render the Cover tab when target="resume"', () => {
      render(<GenerationOutput {...makeProps({ target: 'resume', activeOut: 'resume' })} />);
      expect(
        screen.queryByRole('tab', { name: 'autopilot.apply.target.cover' })
      ).not.toBeInTheDocument();
    });
  });

  // ── 5. aria-selected reflects active tab (tab pattern) ───────────────────────

  describe('aria-selected state', () => {
    it('tabs are grouped in a tablist', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      expect(screen.getByRole('tablist')).toBeInTheDocument();
    });

    it('active doc tab has aria-selected="true", inactive tabs have aria-selected="false"', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);

      // Resume tab is active — aria-selected must be true.
      expect(screen.getByRole('tab', { name: 'autopilot.apply.target.resume' })).toHaveAttribute(
        'aria-selected',
        'true'
      );

      // Cover tab and Job ad tab are inactive.
      expect(screen.getByRole('tab', { name: 'autopilot.apply.target.cover' })).toHaveAttribute(
        'aria-selected',
        'false'
      );

      expect(screen.getByRole('tab', { name: 'autopilot.apply.tabs.jobAd' })).toHaveAttribute(
        'aria-selected',
        'false'
      );
    });

    it('Job ad tab has aria-selected="true" after being clicked', async () => {
      const user = userEvent.setup();
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);

      await clickJobAdTab(user);

      expect(screen.getByRole('tab', { name: 'autopilot.apply.tabs.jobAd' })).toHaveAttribute(
        'aria-selected',
        'true'
      );

      // Doc tabs must now be unselected.
      expect(screen.getByRole('tab', { name: 'autopilot.apply.target.resume' })).toHaveAttribute(
        'aria-selected',
        'false'
      );
    });

    it('switching back to a doc tab sets its aria-selected="true" and Job ad tab to "false"', async () => {
      const user = userEvent.setup();
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);

      // Switch to job ad view.
      await clickJobAdTab(user);

      // Switch back to resume tab.
      await user.click(screen.getByRole('tab', { name: 'autopilot.apply.target.resume' }));

      expect(screen.getByRole('tab', { name: 'autopilot.apply.target.resume' })).toHaveAttribute(
        'aria-selected',
        'true'
      );

      expect(screen.getByRole('tab', { name: 'autopilot.apply.tabs.jobAd' })).toHaveAttribute(
        'aria-selected',
        'false'
      );
    });
  });

  // ── 6. Template picker ────────────────────────────────────────────────────────
  // The single chosen template drives BOTH docs' preview + export, so the picker
  // strip is visible on BOTH doc tabs (résumé AND cover) — never on the job-ad tab.

  describe('Template picker', () => {
    it('renders the template picker on the resume tab (doc view)', () => {
      render(<GenerationOutput {...makeProps({ activeOut: 'resume' })} />);
      expect(screen.getByTestId(TEST_IDS.documents.templatePicker)).toBeInTheDocument();
    });

    it('renders the template picker on the cover tab (doc view)', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'cover' })} />);
      expect(screen.getByTestId(TEST_IDS.documents.templatePicker)).toBeInTheDocument();
    });

    it('is absent after switching to the job-ad view', async () => {
      const user = userEvent.setup();
      render(<GenerationOutput {...makeProps({ activeOut: 'resume' })} />);

      await clickJobAdTab(user);

      expect(screen.queryByTestId(TEST_IDS.documents.templatePicker)).not.toBeInTheDocument();
    });

    it('calls onTemplateChange with the selected id when a two-column template is picked', async () => {
      const user = userEvent.setup();
      const onTemplateChange = vi.fn();
      const onAtsModeChange = vi.fn();
      // Start on a single-column template; pick a two-column one ('atelier').
      render(
        <GenerationOutput
          {...makeProps({
            activeOut: 'resume',
            templateId: 'classic',
            onTemplateChange,
            onAtsModeChange,
          })}
        />
      );

      await pickTemplate(user, 'atelier');

      expect(onTemplateChange).toHaveBeenCalledWith('atelier');
      // Two-column → ATS mode must NOT be forced off.
      expect(onAtsModeChange).not.toHaveBeenCalled();
    });

    it('calls onTemplateChange AND onAtsModeChange(false) when a single-column template is picked', async () => {
      const user = userEvent.setup();
      const onTemplateChange = vi.fn();
      const onAtsModeChange = vi.fn();
      // Start on a two-column template; pick a single-column one ('classic').
      render(
        <GenerationOutput
          {...makeProps({
            activeOut: 'resume',
            templateId: 'atelier',
            onTemplateChange,
            onAtsModeChange,
            atsMode: true,
          })}
        />
      );

      await pickTemplate(user, 'classic');

      expect(onTemplateChange).toHaveBeenCalledWith('classic');
      expect(onAtsModeChange).toHaveBeenCalledWith(false);
    });

    it('does NOT reset ATS mode when Lebenslauf (design tier) is picked', async () => {
      const user = userEvent.setup();
      const onTemplateChange = vi.fn();
      const onAtsModeChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({
            activeOut: 'resume',
            templateId: 'atelier',
            onTemplateChange,
            onAtsModeChange,
            atsMode: true,
          })}
        />
      );

      await pickTemplate(user, 'lebenslauf');

      expect(onTemplateChange).toHaveBeenCalledWith('lebenslauf');
      expect(onAtsModeChange).not.toHaveBeenCalled();
    });
  });

  // ── 7. ATS toggle ─────────────────────────────────────────────────────────────
  // One flag, shown on whichever tab it can still change: the résumé tab when
  // isDesignTier(templateId) (two-column OR photo, incl. Lebenslauf), and the
  // cover tab when the letter layout carries a decoration ATS mode drops.

  describe('ATS toggle', () => {
    it('renders a switch when a two-column template is active on the resume tab', () => {
      // 'atelier' is a confirmed two-column template.
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', templateId: 'atelier' })} />);
      expect(screen.getByRole('switch')).toBeInTheDocument();
    });

    it('renders a switch for Lebenslauf (design-tier, single-column-with-photo)', () => {
      render(
        <GenerationOutput {...makeProps({ activeOut: 'resume', templateId: 'lebenslauf' })} />
      );
      expect(screen.getByRole('switch')).toBeInTheDocument();
    });

    it('does NOT render a switch for an ATS-tier template (classic)', () => {
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', templateId: 'classic' })} />);
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    });

    // Awesome/Deedy are design-tier but neither two-column nor photo-bearing —
    // the toggle hint must NOT claim to remove a photo that doesn't exist (F1).
    it.each(['awesome', 'deedy'] as const)(
      'sets the toggle hint to the decorative-only copy for %s (not the false photo hint)',
      (id) => {
        render(<GenerationOutput {...makeProps({ activeOut: 'resume', templateId: id })} />);
        // The accessible DESCRIPTION, not the hover title: a `title` on the
        // role-less wrapper is never read out, so this is what a screen-reader
        // user actually hears about which document the switch changes.
        expect(screen.getByRole('switch')).toHaveAccessibleDescription(
          'aiGenerate.atsModeHintDecorative'
        );
      }
    );

    it('is absent on the cover tab for a two-column template when the letter is undecorated', () => {
      // The template picker still shows on the cover tab; the résumé's two columns
      // are not what the cover tab's toggle would be about, and the default
      // (classic) letter layout has no decoration to drop.
      render(
        <GenerationOutput
          {...makeProps({ target: 'both', activeOut: 'cover', templateId: 'atelier' })}
        />
      );
      expect(screen.getByTestId(TEST_IDS.documents.templatePicker)).toBeInTheDocument();
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    });

    // ── the cover tab's own gate: a DECORATED letter layout ────────────────────
    // The letter renderer reads the same flag (`data.opts.ats`), so the switch has
    // to be reachable from the cover tab — including under an ATS-tier résumé
    // template, where the letter is the ONLY thing the flag still changes.

    it.each(['banded', 'sidebar', 'monogram'] as const)(
      'renders on the cover tab for the decorated layout %s under an ATS-tier template',
      (letterLayoutId) => {
        render(
          <GenerationOutput
            {...makeProps({
              target: 'both',
              activeOut: 'cover',
              templateId: 'classic',
              letterLayoutId,
            })}
          />
        );
        expect(screen.getByRole('switch')).toHaveAccessibleDescription(
          'aiGenerate.atsModeHintLetter'
        );
      }
    );

    it.each(['classic', 'refined', 'navy'] as const)(
      'is absent on the cover tab for the undecorated layout %s',
      (letterLayoutId) => {
        render(
          <GenerationOutput
            {...makeProps({
              target: 'both',
              activeOut: 'cover',
              templateId: 'classic',
              letterLayoutId,
            })}
          />
        );
        expect(screen.queryByRole('switch')).not.toBeInTheDocument();
      }
    );

    it('uses the résumé hint (not the letter one) back on the résumé tab', () => {
      render(
        <GenerationOutput
          {...makeProps({
            target: 'both',
            activeOut: 'resume',
            templateId: 'atelier',
            letterLayoutId: 'monogram',
          })}
        />
      );
      expect(screen.getByRole('switch')).toHaveAccessibleDescription(
        'aiGenerate.atsModeHintTwoColumn'
      );
      expect(screen.getByRole('switch')).not.toHaveAccessibleDescription(
        'aiGenerate.atsModeHintLetter'
      );
    });

    it('flips atsMode from the cover tab — the reachable off switch for a monogram letter', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({
            target: 'both',
            activeOut: 'cover',
            templateId: 'classic',
            letterLayoutId: 'monogram',
            atsMode: false,
            onAtsModeChange,
          })}
        />
      );

      await user.click(screen.getByRole('switch'));

      expect(onAtsModeChange).toHaveBeenCalledWith(true);
    });

    it('reflects atsMode on the cover tab via aria-checked', () => {
      render(
        <GenerationOutput
          {...makeProps({
            target: 'both',
            activeOut: 'cover',
            templateId: 'classic',
            letterLayoutId: 'monogram',
            atsMode: true,
          })}
        />
      );
      expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
    });

    it('keeps atsMode when an ATS-tier template is picked while the letter is decorated', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({
            target: 'both',
            activeOut: 'cover',
            templateId: 'atelier',
            letterLayoutId: 'monogram',
            atsMode: true,
            onAtsModeChange,
          })}
        />
      );

      // The template dropdown mock renders each option as a role="option" row.
      await user.click(screen.getByRole('option', { name: TEMPLATES['classic'].name }));

      expect(onAtsModeChange).not.toHaveBeenCalled();
    });

    // A silent DOM insertion otherwise: picking Monogram makes the switch appear
    // with nothing announced. The region must be mounted BEFORE the change (a
    // live region cannot announce its own first render), which is what the
    // "empty while hidden" half of this pair pins.
    it('announces the toggle becoming available, via an always-mounted live region', () => {
      const { rerender } = render(
        <GenerationOutput
          {...makeProps({ target: 'both', activeOut: 'cover', templateId: 'classic' })}
        />
      );
      const region = screen.getByRole('status');
      expect(region).toHaveTextContent('');

      rerender(
        <GenerationOutput
          {...makeProps({
            target: 'both',
            activeOut: 'cover',
            templateId: 'classic',
            letterLayoutId: 'monogram',
          })}
        />
      );
      expect(screen.getByRole('status')).toHaveTextContent('aiGenerate.atsToggleAvailable');
    });

    it('still clears atsMode on an ATS-tier pick when the letter layout is undecorated', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({
            target: 'both',
            activeOut: 'cover',
            templateId: 'atelier',
            letterLayoutId: 'navy',
            atsMode: true,
            onAtsModeChange,
          })}
        />
      );

      await user.click(screen.getByRole('option', { name: TEMPLATES['classic'].name }));

      expect(onAtsModeChange).toHaveBeenCalledWith(false);
    });

    it('reflects atsMode=false via aria-checked="false"', () => {
      render(
        <GenerationOutput
          {...makeProps({ activeOut: 'resume', templateId: 'atelier', atsMode: false })}
        />
      );
      expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
    });

    it('reflects atsMode=true via aria-checked="true"', () => {
      render(
        <GenerationOutput
          {...makeProps({ activeOut: 'resume', templateId: 'atelier', atsMode: true })}
        />
      );
      expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
    });

    it('calls onAtsModeChange(!atsMode) when clicked', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({
            activeOut: 'resume',
            templateId: 'atelier',
            atsMode: false,
            onAtsModeChange,
          })}
        />
      );

      await user.click(screen.getByRole('switch'));

      expect(onAtsModeChange).toHaveBeenCalledTimes(1);
      expect(onAtsModeChange).toHaveBeenCalledWith(true);
    });

    it('calls onAtsModeChange(false) when toggled off', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({
            activeOut: 'resume',
            templateId: 'atelier',
            atsMode: true,
            onAtsModeChange,
          })}
        />
      );

      await user.click(screen.getByRole('switch'));

      expect(onAtsModeChange).toHaveBeenCalledWith(false);
    });

    it('is absent for a single-column template (e.g. "classic")', () => {
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', templateId: 'classic' })} />);
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    });

    it('is absent for another single-column template ("classic")', () => {
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', templateId: 'classic' })} />);
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    });
  });

  // ── 7b. Letter layout picker (cover-only) ─────────────────────────────────────
  // The layout picker is the cover-doc counterpart to the résumé-only ATS toggle:
  // it only affects the cover letter, so it renders on the cover tab and never on
  // the résumé or job-ad tabs.

  describe('letter layout picker', () => {
    const letterOption = (id: string) => `${TEST_IDS.generation.letterLayoutOption}-${id}`;

    it('renders on the cover tab', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'cover' })} />);
      expect(screen.getByTestId(letterOption('classic'))).toBeInTheDocument();
    });

    it('is absent on the résumé tab', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      expect(screen.queryByTestId(letterOption('classic'))).not.toBeInTheDocument();
    });

    it('forwards a layout pick to onLetterLayoutChange', async () => {
      const user = userEvent.setup();
      const onLetterLayoutChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({ target: 'both', activeOut: 'cover', onLetterLayoutChange })}
        />
      );
      await user.click(screen.getByTestId(letterOption('refined')));
      expect(onLetterLayoutChange).toHaveBeenCalledWith('refined');
    });

    // Symmetry with the template picker: dropping to an undecorated layout has
    // to RELEASE the shared atsMode, or the next decorated layout returns
    // silently pre-ATS'd and the user exports a monogram-less Monogram letter.
    it('releases atsMode when the new layout is undecorated and nothing else reads it', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({
            target: 'both',
            activeOut: 'cover',
            templateId: 'classic', // ATS-tier → no-op for the résumé
            letterLayoutId: 'monogram',
            atsMode: true,
            onAtsModeChange,
          })}
        />
      );
      await user.click(screen.getByTestId(letterOption('classic')));
      expect(onAtsModeChange).toHaveBeenCalledWith(false);
    });

    it('keeps atsMode on the same change while a design-tier template reads it', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({
            target: 'both',
            activeOut: 'cover',
            templateId: 'atelier', // design-tier → the résumé genuinely uses it
            letterLayoutId: 'monogram',
            atsMode: true,
            onAtsModeChange,
          })}
        />
      );
      await user.click(screen.getByTestId(letterOption('classic')));
      expect(onAtsModeChange).not.toHaveBeenCalled();
    });

    // target='cover': no résumé is exported, so the (design-tier) template must
    // not hold the shared flag open once the letter stops reading it.
    it("target='cover': releases atsMode even under a design-tier template", async () => {
      const user = userEvent.setup();
      const onAtsModeChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({
            target: 'cover',
            activeOut: 'cover',
            templateId: 'atelier',
            letterLayoutId: 'monogram',
            atsMode: true,
            onAtsModeChange,
          })}
        />
      );
      await user.click(screen.getByTestId(letterOption('classic')));
      expect(onAtsModeChange).toHaveBeenCalledWith(false);
    });

    it('does not release atsMode when swapping between two DECORATED layouts', async () => {
      const user = userEvent.setup();
      const onAtsModeChange = vi.fn();
      render(
        <GenerationOutput
          {...makeProps({
            target: 'both',
            activeOut: 'cover',
            templateId: 'classic',
            letterLayoutId: 'monogram',
            atsMode: true,
            onAtsModeChange,
          })}
        />
      );
      await user.click(screen.getByTestId(letterOption('sidebar')));
      expect(onAtsModeChange).not.toHaveBeenCalled();
    });
  });

  // ── 8. Edit → save → preview committed-text logic ────────────────────────────
  // The component is controlled: onEdit informs the parent which passes the new
  // output back down. ControlledWrapper simulates that round-trip.
  // PdfPreview (inside previewSlot) always renders the COMMITTED text, which now
  // auto-commits via a ~700 ms debounce — no manual Save button.

  describe('Edit → debounce → preview flow', () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => vi.useRealTimers());

    it('preview text matches the initial output on first render', () => {
      render(
        <ControlledWrapper
          {...makeProps({ activeOut: 'resume', output: 'Initial content', editable: true })}
        />
      );
      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveTextContent(
        'Initial content'
      );
    });

    it('a parent-driven output change (no local edit) refreshes the preview immediately', () => {
      const props = makeProps({ activeOut: 'resume', output: 'Version 1', editable: true });
      const { rerender } = render(<GenerationOutput {...props} />);

      // No local edit — external change must update committed immediately.
      rerender(<GenerationOutput {...props} output="Version 2" />);

      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveTextContent('Version 2');
    });

    it('before 700 ms the preview still shows the last committed text', () => {
      render(
        <ControlledWrapper
          {...makeProps({ activeOut: 'resume', output: 'Committed text', editable: true })}
        />
      );

      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveTextContent('Committed text');

      const editBox = screen.getByTestId(TEST_IDS.documents.editableInput);
      void act(() => {
        editBox.textContent = 'Edited text';
        editBox.dispatchEvent(new Event('input', { bubbles: true }));
      });

      void act(() => vi.advanceTimersByTime(699));

      // Preview must still show old committed text — debounce not yet fired.
      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveTextContent('Committed text');
    });

    it('after 700 ms the debounce auto-commits and the preview updates', () => {
      render(
        <ControlledWrapper
          {...makeProps({ activeOut: 'resume', output: 'Old text', editable: true })}
        />
      );

      const editBox = screen.getByTestId(TEST_IDS.documents.editableInput);
      void act(() => {
        editBox.textContent = 'New text';
        editBox.dispatchEvent(new Event('input', { bubbles: true }));
      });

      void act(() => vi.advanceTimersByTime(700));

      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveTextContent('New text');
    });

    it('blur flushes the debounce immediately without waiting 700 ms', async () => {
      // Switch to real timers for this test — the blur flush is synchronous and
      // fake-timer + async-act interaction can hide the state update.
      vi.useRealTimers();

      render(
        <ControlledWrapper
          {...makeProps({ activeOut: 'resume', output: 'Before blur', editable: true })}
        />
      );

      const editBox = screen.getByTestId(TEST_IDS.documents.editableInput);
      await act(async () => {
        editBox.textContent = 'After blur';
        editBox.dispatchEvent(new Event('input', { bubbles: true }));
      });
      await act(async () => {
        editBox.dispatchEvent(new Event('blur', { bubbles: true }));
      });

      // Blur flushes the commit synchronously; waitFor handles React's async render.
      await waitFor(() => {
        expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveTextContent('After blur');
      });
    });

    it('no Save button is rendered', () => {
      render(
        <ControlledWrapper
          {...makeProps({ activeOut: 'resume', output: 'Some text', editable: true })}
        />
      );
      expect(screen.queryByTestId(TEST_IDS.documents.saveBtn)).not.toBeInTheDocument();
    });

    // ── BUG 2 regression: tab-switch commit must route to the correct doc ─────────
    // Uses the REAL useDebouncedCommit hook (not mocked) + fake timers.
    // Scenario: type on resume tab → switch to cover before 700 ms → the flush
    // triggered by the switch must commit the typed value to RESUME (not cover);
    // cover's preview must remain unchanged.
    it('edit resume → switch tab before 700 ms → resume commits typed value, cover is untouched', () => {
      // Stateful wrapper that mirrors the real parent: each doc owns its own output
      // string and the active doc's output is passed down. Switching tabs passes
      // the COVER's content as output, so the external-change detection does not
      // accidentally overwrite committed.cover with the resume text.
      function TabSwitchWrapper() {
        const [activeOut, setActiveOut] = React.useState<'resume' | 'cover'>('resume');
        const [resumeText, setResumeText] = React.useState('Original resume');
        const coverText = 'Cover content';
        const output = activeOut === 'resume' ? resumeText : coverText;
        const handleEdit = (text: string) => {
          if (activeOut === 'resume') setResumeText(text);
        };
        return (
          <GenerationOutput
            {...makeProps({
              target: 'both',
              activeOut,
              setActiveOut,
              output,
              onEdit: handleEdit,
              editable: true,
            })}
          />
        );
      }

      render(<TabSwitchWrapper />);

      // 1. Type on the resume tab — scheduleCommit('resume', 'Typed resume') fires.
      const editBox = screen.getByTestId(TEST_IDS.documents.editableInput);
      void act(() => {
        editBox.textContent = 'Typed resume';
        editBox.dispatchEvent(new Event('input', { bubbles: true }));
      });

      // 2. Advance only 300 ms — debounce has NOT fired yet.
      void act(() => vi.advanceTimersByTime(300));

      // Preview still shows old committed text (resume).
      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveTextContent(
        'Original resume'
      );

      // 3. Switch to cover tab before the 700 ms window — triggers flush().
      //    flush() must commit ('resume', 'Typed resume') — not ('cover', anything).
      void act(() => {
        screen.getByRole('tab', { name: 'autopilot.apply.target.cover' }).click();
      });

      // 4. Advance past the original debounce window; the timer was cancelled by flush.
      void act(() => vi.advanceTimersByTime(700));

      // Cover tab is now active — its committed text comes from coverText ('Cover content').
      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveTextContent('Cover content');

      // 5. Switch BACK to resume to verify its committed value.
      void act(() => {
        screen.getByRole('tab', { name: 'autopilot.apply.target.resume' }).click();
      });

      // Resume must show the typed value — committed by the flush at tab-switch time.
      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveTextContent('Typed resume');
    });
  });

  // ── 8b. Export/preview market → PdfPreview's `locale` ────────────────────────
  // Regression guard: `useTailorPipeline` resolves the export market and passes
  // it as `market`, but the live preview used to receive no locale at all (the
  // Rust exporter then silently falls back to market "intl" for the preview
  // while the real export uses the resolved one — a German posting showed an
  // English salutation on screen but a German one in the download). Asserting
  // the actual forwarded value, not just that a render happened, is what would
  // fail if this prop were ever dropped again.

  describe('Export/preview market', () => {
    it('forwards a German market to PdfPreview as `locale` (not undefined)', () => {
      render(<GenerationOutput {...makeProps({ market: 'de' })} />);
      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveAttribute(
        'data-locale',
        'de'
      );
    });

    it('leaves `locale` empty when no market was resolved', () => {
      render(<GenerationOutput {...makeProps({ market: undefined })} />);
      expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveAttribute('data-locale', '');
    });
  });

  // ── 9. Tabpanel ARIA linkage ──────────────────────────────────────────────
  // The single `role="tabpanel"` region must carry `id`, `aria-labelledby`,
  // and `aria-controls` wired to the ACTIVE tab. The active tab must carry a
  // matching `aria-controls` pointing to the panel id.

  describe('Tabpanel ARIA linkage', () => {
    it('tabpanel has role="tabpanel" with a non-empty id', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      const panel = screen.getByRole('tabpanel');
      expect(panel).toBeInTheDocument();
      expect(panel.id).toBeTruthy();
    });

    it('tabpanel id is "tailor-panel-resume" when resume tab is active', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      expect(screen.getByRole('tabpanel').id).toBe('tailor-panel-resume');
    });

    it('tabpanel id is "tailor-panel-cover" when cover tab is active', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'cover' })} />);
      expect(screen.getByRole('tabpanel').id).toBe('tailor-panel-cover');
    });

    it('tabpanel id is "tailor-panel-jobad" when the Job ad tab is active', async () => {
      const user = userEvent.setup();
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);

      await clickJobAdTab(user);

      expect(screen.getByRole('tabpanel').id).toBe('tailor-panel-jobad');
    });

    it('tabpanel aria-labelledby matches the id of the active tab', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      const panel = screen.getByRole('tabpanel');
      const labelledBy = panel.getAttribute('aria-labelledby');
      expect(labelledBy).toBe('tailor-tab-resume');
      // The tab element with that id must exist.
      expect(document.getElementById('tailor-tab-resume')).toBeInTheDocument();
    });

    it('active resume tab aria-controls points to the panel id', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      const resumeTab = screen.getByRole('tab', { name: 'autopilot.apply.target.resume' });
      expect(resumeTab.getAttribute('aria-controls')).toBe('tailor-panel-resume');
    });

    it('active cover tab aria-controls points to the panel id', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'cover' })} />);
      const coverTab = screen.getByRole('tab', { name: 'autopilot.apply.target.cover' });
      expect(coverTab.getAttribute('aria-controls')).toBe('tailor-panel-cover');
    });

    it('job ad tab aria-controls points to "tailor-panel-jobad"', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      const jobAdTab = screen.getByRole('tab', { name: 'autopilot.apply.tabs.jobAd' });
      expect(jobAdTab.getAttribute('aria-controls')).toBe('tailor-panel-jobad');
    });

    it('tabpanel aria-labelledby updates to the job ad tab id after switching to job ad view', async () => {
      const user = userEvent.setup();
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);

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

      await user.click(screen.getByRole('tab', { name: 'autopilot.apply.target.cover' }));

      const panel = screen.getByRole('tabpanel');
      expect(panel.getAttribute('aria-labelledby')).toBe('tailor-tab-cover');
      expect(panel.id).toBe('tailor-panel-cover');
    });

    it('tabpanel has tabIndex={0} for keyboard reachability', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      expect(screen.getByRole('tabpanel')).toHaveAttribute('tabindex', '0');
    });
  });

  // ── 10. Scroll boundary — the tab/action header stays pinned ─────────────────
  // Regression guard for the "header scrolls away" bug: the viewer used to grow
  // past its host (intrinsic `min-h-[32rem]` panel + no overflow of its own), so
  // the PARENT scrolled the whole component — header included. The scroll boundary
  // now lives on the tabpanel: the tab/action bar is a pinned `shrink-0` sibling
  // OUTSIDE it, while the option strips live INSIDE and scroll with the document.
  // jsdom has no layout, so these assert the structural invariants that produce the
  // behaviour, not pixels — the pixel measurements come from the Chromium run
  // recorded in the handoff.

  describe('scroll boundary', () => {
    const SCROLLS = /overflow-(?:y-)?(?:auto|scroll)/;

    it('the tabpanel owns the vertical scroll', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      expect(screen.getByRole('tabpanel').className).toMatch(SCROLLS);
    });

    it('the tabpanel is bounded by its host, with no intrinsic min-height', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      const panel = screen.getByRole('tabpanel');
      expect(panel.className).toContain('min-h-0');
      expect(panel.className).toContain('flex-1');
      // An arbitrary min-height (e.g. `min-h-[32rem]`) makes the panel taller than
      // its host again, which pushes the scroll back up to the caller.
      expect(panel.className).not.toMatch(/min-h-(?!0\b)/);
    });

    it('the root is height-bounded so the caller never has to scroll it', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      const root = screen.getByRole('tabpanel').parentElement;
      expect(root).not.toBeNull();
      expect(root?.className).toContain('min-h-0');
      expect(root?.className).toContain('flex-1');
      expect(root?.className).toContain('overflow-hidden');
      expect(root?.className).not.toMatch(/min-h-(?!0\b)/);
    });

    it('gives the document region a floor so the scrollport can actually engage', () => {
      // Without a floor every child is flex-1/h-full, the content fits the
      // scrollport exactly and `overflow-y-auto` can never fire.
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      const region = screen.getByTestId(TEST_IDS.documents.documentRegion);
      expect(region.className).toMatch(/min-h-\[\d+rem\]/);
      expect(region.className).toContain('flex-1');
      expect(screen.getByRole('tabpanel').contains(region)).toBe(true);
    });

    it('nothing between the root and the tabpanel is a second scroll container', () => {
      const { container } = render(
        <GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />
      );
      for (
        let el = screen.getByRole('tabpanel').parentElement;
        el !== null && el !== container;
        el = el.parentElement
      ) {
        expect(el.className).not.toMatch(SCROLLS);
      }
    });

    it('keeps the tabs and the Copy/Export actions outside the scrollport', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'resume' })} />);
      const panel = screen.getByRole('tabpanel');
      expect(panel.contains(screen.getByRole('tablist'))).toBe(false);
      expect(panel.contains(screen.getByRole('button', { name: /autopilot\.apply\.copy/i }))).toBe(
        false
      );
      expect(panel.contains(screen.getByRole('button', { name: /aiGenerate\.export/i }))).toBe(
        false
      );
    });

    it('scrolls the template / accent / letter-layout strips WITH the document', () => {
      // Pinning these costs more permanent chrome than a small window can spare:
      // the document collapses to nothing and the last strip is clipped out of
      // reach behind the root's overflow-hidden. They belong in the scrollport.
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'cover' })} />);
      const panel = screen.getByRole('tabpanel');
      expect(panel.contains(screen.getByTestId(TEST_IDS.documents.templatePicker))).toBe(true);
      expect(
        panel.contains(screen.getByTestId(`${TEST_IDS.generation.letterLayoutOption}-classic`))
      ).toBe(true);
    });
  });

  // ── Quality badge — cold-hydrated report + staleness (Phase-1 finding #3) ────
  // `output` (the doc GenerationOutput actually renders) is what a cold-entry
  // hydration seeds into the session's resumeOut/coverOut from a persisted
  // record's `resumeText`/`coverLetterText` — the same string a real
  // `parseQualityReport(seedGeneration.qualityReport)` was hashed against at
  // save time. This exercises the REAL QualityBadge (no stub), the level the
  // staleness comparison actually renders at.
  describe('quality badge — seeded report + staleness', () => {
    const OUTPUT = 'Generated resume content'; // matches makeProps()'s default `output`
    const PAYLOAD = {
      ok: true,
      issues: [],
      metrics: {
        keywordCoverage: 80,
        topRequirementHits: 1,
        duplicateRatio: 0,
        rolesSource: 1,
        rolesOutput: 1,
      },
    };
    const REPORT: QualityReport = {
      schemaVersion: 2,
      pipeline: 'fast',
      generatedAt: 1,
      resume: { report: PAYLOAD, sourceTextHash: hashText(OUTPUT) },
    };
    const STALE: QualityReport = {
      ...REPORT,
      resume: { report: PAYLOAD, sourceTextHash: hashText('DIFFERENT') },
    };

    it('renders the badge for a seeded, unedited report (hash matches — not stale)', () => {
      render(<GenerationOutput {...makeProps({ report: REPORT })} />);
      expect(screen.getByRole('button', { name: /quality\.badge\.clean/ })).toBeInTheDocument();
    });

    it('renders the stale state once the hash no longer matches (edited since hydration)', () => {
      render(<GenerationOutput {...makeProps({ report: STALE })} />);
      expect(screen.getByRole('button', { name: /quality\.badge\.stale/ })).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /quality\.badge\.clean/ })).toBeNull();
    });

    it('renders nothing when there is no report yet', () => {
      render(<GenerationOutput {...makeProps({ report: null })} />);
      expect(screen.queryByRole('button', { name: /quality\.badge/ })).toBeNull();
    });

    // This is the ONE surface with inline editing, so it is the only one whose
    // badge can go stale mid-session — it must also offer the way back out.
    it('offers Re-check in the panel when the host wires it', async () => {
      const user = userEvent.setup();
      const onRecheck = vi.fn();
      render(<GenerationOutput {...makeProps({ report: STALE, onRecheck })} />);

      await user.click(screen.getByRole('button', { name: /quality\.badge\.stale/ }));
      await user.click(screen.getByRole('button', { name: /quality\.panel\.recheck/ }));
      expect(onRecheck).toHaveBeenCalledTimes(1);
    });

    it('hides Re-check when the host cannot supply it', async () => {
      const user = userEvent.setup();
      render(<GenerationOutput {...makeProps({ report: STALE })} />);

      await user.click(screen.getByRole('button', { name: /quality\.badge\.stale/ }));
      expect(screen.queryByRole('button', { name: /quality\.panel\.recheck/ })).toBeNull();
    });
  });

  // ── 11. Score strip — résumé result surfaces the job-match score ─────────────
  // Real render-logic guards (isMeasured/hasScoreCoverage/ScoreMetric) are
  // covered once, at the source, by JobAdView.i18n.test.tsx against REAL
  // translated copy — that module is now shared (MatchScoreMetric.tsx), not
  // forked. This block covers GenerationScoreStrip's OWN wiring: which tab it
  // renders on, and that it never fabricates a `0`.

  describe('Score strip', () => {
    // Clears accumulated calls from every earlier test in this file so the
    // "which text did the LATEST render call the hook with" test below finds
    // this test's own call, not some earlier test's.
    beforeEach(() => {
      mockUseJobAdTextMatchScore.mockClear();
    });

    function baseScore(overrides: Partial<MatchScore> = {}): MatchScore {
      return {
        resumeId: 'resume-1',
        jobId: 'job-1',
        ats: 72,
        semantic: 0,
        combined: 72,
        gaps: ['docker'],
        recommendations: [],
        scoreSource: 'keyword',
        ...overrides,
      };
    }

    it('renders a real percentage when the score is measured', () => {
      stubbedScore = { data: baseScore({ ats: 72 }), isLoading: false };
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />);
      expect(screen.getByTestId(TEST_IDS.documents.scoreStripCoverage)).toHaveTextContent('72%');
    });

    it('renders the stated reason, never "0%", for the no-extractable-keywords placeholder', () => {
      stubbedScore = {
        data: baseScore({ ats: 0, combined: 0, gaps: [] }),
        isLoading: false,
      };
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />);
      expect(screen.getByTestId(TEST_IDS.documents.scoreStripCoverage)).toHaveTextContent(
        'autopilot.apply.jobAdView.score.noKeywords'
      );
      expect(screen.queryByText('0%')).not.toBeInTheDocument();
    });

    it('shows the no-résumé reason (never a score) when no resumeId is threaded', () => {
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: undefined })} />);
      expect(screen.getByTestId(TEST_IDS.documents.scoreStrip)).toHaveTextContent(
        'jobs.scoreNoResume'
      );
      expect(screen.queryByTestId(TEST_IDS.documents.scoreStripCoverage)).not.toBeInTheDocument();
    });

    it('does NOT render on the cover-letter tab — a cover letter is not scored against keyword coverage', () => {
      stubbedScore = { data: baseScore(), isLoading: false };
      render(
        <GenerationOutput
          {...makeProps({ target: 'both', activeOut: 'cover', resumeId: 'resume-1' })}
        />
      );
      expect(screen.queryByTestId(TEST_IDS.documents.scoreStrip)).not.toBeInTheDocument();
    });

    it('does NOT render on the job-ad tab', async () => {
      const user = userEvent.setup();
      stubbedScore = { data: baseScore(), isLoading: false };
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />);

      await clickJobAdTab(user);

      expect(screen.queryByTestId(TEST_IDS.documents.scoreStrip)).not.toBeInTheDocument();
    });

    it('passes the snapshotted jobDesc (not a live-editable value) as the query text argument', () => {
      stubbedScore = { data: baseScore(), isLoading: false };
      render(
        <GenerationOutput
          {...makeProps({
            activeOut: 'resume',
            resumeId: 'resume-1',
            jobDesc: 'Snapshot-worthy posting text',
          })}
        />
      );
      const enabledCall = mockUseJobAdTextMatchScore.mock.calls.find((call) => call[2] === true);
      expect(enabledCall?.[0]).toBe('resume-1');
      expect(enabledCall?.[1]).toBe('Snapshot-worthy posting text');
    });

    // ── Loading / error / malformed-payload branches ─────────────────────────
    // GenerationOutput.test.tsx previously covered only measured / placeholder /
    // no-résumé / tab-gating — these are the branches that enforce the honesty
    // guarantee itself (never a fabricated `0%`/score for a request that hasn't
    // resolved, failed, or came back malformed).

    it('announces loading via a live region', () => {
      stubbedScore = { data: undefined, isLoading: true };
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />);

      const strip = screen.getByTestId(TEST_IDS.documents.scoreStrip);
      expect(strip).toHaveAttribute('role', 'status');
      expect(strip).toHaveAttribute('aria-live', 'polite');
      expect(strip).toHaveTextContent('autopilot.apply.jobAdView.score.loading');
    });

    it('shows an alert with a working retry on a rejected request', async () => {
      const user = userEvent.setup();
      const refetch = vi.fn();
      stubbedScore = { data: undefined, isLoading: false, isError: true, refetch };
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />);

      const strip = screen.getByTestId(TEST_IDS.documents.scoreStrip);
      expect(strip).toHaveAttribute('role', 'alert');
      expect(strip).toHaveTextContent('autopilot.apply.jobAdView.score.errorTitle');

      await user.click(screen.getByRole('button', { name: /autopilot\.apply\.tryAgain/i }));
      expect(refetch).toHaveBeenCalledTimes(1);
    });

    it('renders the same alert (never a fabricated score) for a resolved-but-malformed payload', () => {
      // `invoke()` never validates the resolved shape — a failure response
      // (e.g. a résumé id that outlived its résumé) can resolve typed as
      // MatchScore while missing ats/combined/gaps/recommendations.
      stubbedScore = {
        data: { resumeId: 'resume-1', jobId: 'job-1' },
        isLoading: false,
      };
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />);

      const strip = screen.getByTestId(TEST_IDS.documents.scoreStrip);
      expect(strip).toHaveAttribute('role', 'alert');
      expect(strip).toHaveTextContent('autopilot.apply.jobAdView.score.errorTitle');
      expect(screen.queryByText('0%')).not.toBeInTheDocument();
      expect(screen.queryByText(/NaN%/)).not.toBeInTheDocument();
    });

    // ── CLI-agent egress disclosure (mirrors JobAdView's Score tab) ──────────
    // Unlike the Score tab, this strip fires ON MOUNT whenever a résumé is
    // threaded — no explicit click required — so the disclosure matters even
    // more here.

    it('discloses CLI-agent egress while loading and on the error branch, never for a local provider', () => {
      mockActiveProvider = 'claude-code';
      stubbedScore = { data: undefined, isLoading: true };
      const { rerender } = render(
        <GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />
      );
      expect(screen.getByTestId(TEST_IDS.documents.scoreStrip)).toHaveTextContent(
        'autopilot.apply.jobAdView.score.cliAgentEgress'
      );

      stubbedScore = { data: undefined, isLoading: false, isError: true, refetch: vi.fn() };
      rerender(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />);
      expect(screen.getByTestId(TEST_IDS.documents.scoreStrip)).toHaveTextContent(
        'autopilot.apply.jobAdView.score.cliAgentEgress'
      );

      mockActiveProvider = 'ollama';
      rerender(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />);
      expect(screen.getByTestId(TEST_IDS.documents.scoreStrip)).not.toHaveTextContent(
        'autopilot.apply.jobAdView.score.cliAgentEgress'
      );
    });

    it('does NOT disclose egress on the no-résumé reason — nothing was ever sent', () => {
      mockActiveProvider = 'claude-code';
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: undefined })} />);
      expect(screen.getByTestId(TEST_IDS.documents.scoreStrip)).not.toHaveTextContent(
        'autopilot.apply.jobAdView.score.cliAgentEgress'
      );
    });

    // ── Finding 1 regression: the snapshot must survive the strip unmounting ──
    // The strip only renders on `view === 'doc' && activeOut === 'resume'`, so
    // switching to the Job ad tab unmounts it. The snapshot MUST be owned by
    // GenerationOutput (which stays mounted) — a snapshot re-initialised on
    // the strip's own remount would silently score the EDITED posting text,
    // through a path that can route via translation.

    it('a tab switch away and back still scores the ORIGINAL snapshot, not a jobDesc edited while away', async () => {
      const user = userEvent.setup();
      stubbedScore = { data: baseScore(), isLoading: false };
      const props = makeProps({
        activeOut: 'resume',
        resumeId: 'resume-1',
        jobDesc: 'Original posting text',
      });
      const { rerender } = render(<GenerationOutput {...props} />);

      // Switch to the Job ad tab — the strip unmounts.
      await clickJobAdTab(user);
      expect(screen.queryByTestId(TEST_IDS.documents.scoreStrip)).not.toBeInTheDocument();

      // Simulate the posting being edited on the Job ad sub-tab while the
      // strip is unmounted — GenerationOutput is controlled, so the parent
      // would pass this back down as a new `jobDesc`.
      rerender(<GenerationOutput {...props} jobDesc="Edited posting text" />);

      // Switch back to the résumé tab — the strip remounts.
      await user.click(screen.getByRole('tab', { name: 'autopilot.apply.target.resume' }));

      const enabledCalls = mockUseJobAdTextMatchScore.mock.calls.filter((call) => call[2] === true);
      const lastEnabledCall = enabledCalls[enabledCalls.length - 1];
      expect(lastEnabledCall?.[1]).toBe('Original posting text');
    });
  });
});
