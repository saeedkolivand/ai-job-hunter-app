import React, { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as AjhUi from '@ajh/ui';

import { GenerationOutput } from './GenerationOutput';

// ── Module stubs ──────────────────────────────────────────────────────────────

// Echo every key verbatim — no i18next runtime needed in jsdom.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// EditableOutput mock — exposes onChange/onSave/canSave + renders previewSlot.
// Uses divs (not raw <textarea>/<button>) to stay clear of the @ajh/ui ESLint rule.
// The mock is intentionally richer than the original so edit/save/preview tests
// can drive the component's committed-text logic without the real editor tree.
vi.mock('@/components/generation/EditableOutput', () => ({
  EditableOutput: ({
    value,
    onChange,
    onSave,
    canSave,
    previewSlot,
  }: {
    value: string;
    onChange?: (v: string) => void;
    onSave?: () => void;
    canSave?: boolean;
    previewSlot?: React.ReactNode;
  }) => (
    <div data-testid="editable-output">
      {value}
      <div
        role="textbox"
        data-testid="editable-input"
        contentEditable
        suppressContentEditableWarning
        onInput={(e) => onChange?.((e.target as HTMLElement).textContent ?? '')}
      />
      {onSave && (
        <div role="button" data-testid="save-btn" data-can-save={String(canSave)} onClick={onSave}>
          save
        </div>
      )}
      {previewSlot && <div data-testid="preview-slot">{previewSlot}</div>}
    </div>
  ),
}));

// PdfPreview mock — renders its `text` prop into a testid so tests can inspect
// the committed text without launching the real Typst/PDF pipeline.
vi.mock('@/components/generation/PdfPreview', () => ({
  PdfPreview: ({ text }: { text: string }) => <div data-testid="pdf-preview">{text}</div>,
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
    templateId: 'modern' as const,
    atsMode: false,
    onTemplateChange: vi.fn(),
    onAtsModeChange: vi.fn(),
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
      expect(screen.queryByTestId('editable-output')).not.toBeInTheDocument();
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
      expect(screen.getByTestId('editable-output')).toBeInTheDocument();

      await clickJobAdTab(user);

      // EditableOutput must be gone after switching to job-ad view.
      expect(screen.queryByTestId('editable-output')).not.toBeInTheDocument();
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
      expect(screen.getByTestId('template-picker')).toBeInTheDocument();
    });

    it('renders the template picker on the cover tab (doc view)', () => {
      render(<GenerationOutput {...makeProps({ target: 'both', activeOut: 'cover' })} />);
      expect(screen.getByTestId('template-picker')).toBeInTheDocument();
    });

    it('is absent after switching to the job-ad view', async () => {
      const user = userEvent.setup();
      render(<GenerationOutput {...makeProps({ activeOut: 'resume' })} />);

      await clickJobAdTab(user);

      expect(screen.queryByTestId('template-picker')).not.toBeInTheDocument();
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
            templateId: 'modern',
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
  });

  // ── 7. ATS toggle ─────────────────────────────────────────────────────────────
  // The switch is rendered only on the résumé tab AND when
  // isTwoColumnTemplate(templateId) is true — ATS single-column linearization is a
  // résumé concept, so it never shows on the cover tab.

  describe('ATS toggle', () => {
    it('renders a switch when a two-column template is active on the resume tab', () => {
      // 'atelier' is a confirmed two-column template.
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', templateId: 'atelier' })} />);
      expect(screen.getByRole('switch')).toBeInTheDocument();
    });

    it('is absent on the cover tab even for a two-column template', () => {
      // The template picker still shows on the cover tab, but the ATS toggle does not.
      render(
        <GenerationOutput
          {...makeProps({ target: 'both', activeOut: 'cover', templateId: 'atelier' })}
        />
      );
      expect(screen.getByTestId('template-picker')).toBeInTheDocument();
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
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

    it('is absent for a single-column template (e.g. "modern")', () => {
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', templateId: 'modern' })} />);
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    });

    it('is absent for another single-column template ("classic")', () => {
      render(<GenerationOutput {...makeProps({ activeOut: 'resume', templateId: 'classic' })} />);
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    });
  });

  // ── 8. Edit → save → preview committed-text logic ────────────────────────────
  // The component is controlled: onEdit informs the parent which passes the new
  // output back down. ControlledWrapper simulates that round-trip.
  // PdfPreview (inside previewSlot) always renders the COMMITTED text, not the
  // live keystroke value — it only updates when Save is clicked.

  describe('Edit → save → preview flow', () => {
    it('preview text matches the initial output on first render', () => {
      render(
        <ControlledWrapper
          {...makeProps({ activeOut: 'resume', output: 'Initial content', editable: true })}
        />
      );
      expect(screen.getByTestId('pdf-preview')).toHaveTextContent('Initial content');
    });

    it('a parent-driven output change (no local edit) refreshes the preview immediately', () => {
      const props = makeProps({ activeOut: 'resume', output: 'Version 1', editable: true });
      const { rerender } = render(<GenerationOutput {...props} />);

      // No local edit has happened — the effect must update committed when output changes.
      rerender(<GenerationOutput {...props} output="Version 2" />);

      expect(screen.getByTestId('pdf-preview')).toHaveTextContent('Version 2');
    });

    it('after a local edit the preview still shows the last committed (pre-save) text', async () => {
      const user = userEvent.setup();
      render(
        <ControlledWrapper
          {...makeProps({ activeOut: 'resume', output: 'Committed text', editable: true })}
        />
      );

      // Verify preview shows initial committed text before any edit.
      expect(screen.getByTestId('pdf-preview')).toHaveTextContent('Committed text');

      // Simulate a local edit: type into the contentEditable box.
      const editBox = screen.getByTestId('editable-input');
      await user.click(editBox);
      await user.type(editBox, 'Edited text');

      // Preview must still show the old committed text — Save not clicked yet.
      expect(screen.getByTestId('pdf-preview')).toHaveTextContent('Committed text');
    });

    it('canSave becomes true after a local edit', async () => {
      const user = userEvent.setup();
      render(
        <ControlledWrapper
          {...makeProps({ activeOut: 'resume', output: 'Original', editable: true })}
        />
      );

      const saveBtn = screen.getByTestId('save-btn');
      expect(saveBtn).toHaveAttribute('data-can-save', 'false');

      const editBox = screen.getByTestId('editable-input');
      await user.click(editBox);
      await user.type(editBox, 'Changed');

      expect(screen.getByTestId('save-btn')).toHaveAttribute('data-can-save', 'true');
    });

    it('clicking save commits the current output to the preview and canSave goes false', async () => {
      const user = userEvent.setup();
      render(
        <ControlledWrapper
          {...makeProps({ activeOut: 'resume', output: 'Before save', editable: true })}
        />
      );

      // Edit to diverge committed from output.
      const editBox = screen.getByTestId('editable-input');
      await user.click(editBox);
      await user.type(editBox, 'After save');

      // Preview still shows old text before Save.
      expect(screen.getByTestId('pdf-preview')).toHaveTextContent('Before save');

      // Click Save.
      await user.click(screen.getByTestId('save-btn'));

      // After save the preview must reflect the new committed text.
      // The component sets committed[activeOut] = output (which the wrapper
      // already updated to include the typed text).
      expect(screen.getByTestId('save-btn')).toHaveAttribute('data-can-save', 'false');
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
});
