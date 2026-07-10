import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { OutputPanelDone } from './index';

// Stub the real-PDF preview (#24) — it renders the export via IPC, out of scope
// for this panel's preview/edit wiring test (covered in PdfPreview's own suite).
// The received `letterLayoutId` is surfaced as a data-* attribute so tests can
// prove the preview reads the SAME value threaded to export (ADR-012 parity).
vi.mock('@/components/generation/PdfPreview', () => ({
  PdfPreview: ({ letterLayoutId }: { letterLayoutId?: string }) => (
    <div data-testid={TEST_IDS.documents.pdfPreview} data-letter-layout-id={letterLayoutId ?? ''}>
      PDF
    </div>
  ),
}));

// EditableOutput (rendered inside OutputPanelDone) calls useContactProfile() which
// reaches for AppClientProvider.  Return a stub so no provider tree is needed.
vi.mock('@/services/use-contact-profile', () => ({
  useContactProfile: () => ({ data: undefined }),
}));

// Stub useDebouncedCommit so tests don't depend on fake timers.
// scheduleCommit immediately calls onCommit with the (out, value) pair —
// simulates instant commit in tests. flush() with no argument is also a no-op
// (the pair was just committed by scheduleCommit already).
vi.mock('@/hooks/use-debounced-commit', () => ({
  useDebouncedCommit: (onCommit: (out: string, v: string) => void) => ({
    scheduleCommit: (out: string, v: string) => onCommit(out, v),
    flush: () => undefined,
    cancel: () => undefined,
  }),
}));

const RAW = 'Led **payments** migration at scale.';

function renderPanel(overrides: Partial<React.ComponentProps<typeof OutputPanelDone>> = {}) {
  const onOutputChange = vi.fn();
  const onExport = vi.fn();
  const onCopy = vi.fn();
  const onLetterLayoutChange = vi.fn();
  render(
    <OutputPanelDone
      resumeOut={RAW}
      coverOut=""
      activeOut="resume"
      meta={null}
      mode="ats"
      templateId="classic"
      atsMode={false}
      onActiveOutChange={vi.fn()}
      onLetterLayoutChange={onLetterLayoutChange}
      onCopy={onCopy}
      onExport={onExport}
      onOutputChange={onOutputChange}
      onRegenerate={vi.fn()}
      copied={false}
      {...overrides}
    />
  );
  return { onOutputChange, onExport, onCopy, onLetterLayoutChange };
}

describe('OutputPanelDone — preview/edit', () => {
  it('shows the real-PDF preview by default (#24), not markdown or a textarea', () => {
    renderPanel();
    // The default Preview tab renders the real-PDF view, not the markdown fallback.
    expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toBeInTheDocument();
    expect(screen.queryByText(/\*\*payments\*\*/)).toBeNull();
    // No editable textarea while previewing.
    expect(screen.queryByRole('textbox')).toBeNull();
  });

  it('switches to a raw textarea with markers intact (export source untouched)', () => {
    const { onOutputChange } = renderPanel();
    // The Preview/Edit/Source switch is a SegmentedControl radio group. The raw
    // markdown textarea lives in the **Source** tab (Edit is now the WYSIWYG surface).
    // t('aiGenerate.source') resolves to "Source" via the real en locale.
    fireEvent.click(screen.getByRole('radio', { name: /source/i }));

    const textarea = screen.getByRole<HTMLTextAreaElement>('textbox');
    // Raw text — including the **payments** markers the export pipeline reads.
    expect(textarea.value).toBe(RAW);
    // Switching views must not mutate the canonical output.
    expect(onOutputChange).not.toHaveBeenCalled();
  });

  it('no Save button is rendered (auto-debounce replaced manual save)', () => {
    renderPanel();
    // Switch to Source so the full edit toolbar is visible.
    fireEvent.click(screen.getByRole('radio', { name: /source/i }));
    expect(screen.queryByRole('button', { name: /save/i })).toBeNull();
  });
});

// The primary AI-Generate flow defaults `target: 'both'`, producing BOTH docs —
// the letter-layout picker must be reachable there (regression: it was only
// wired via StepTemplate's cover-only gate, locking `target: 'both'` users to
// Classic since OutputPanelDone never rendered it).
describe('OutputPanelDone — letter-layout picker (cover tab only)', () => {
  const letterOption = (id: string) => `${TEST_IDS.generation.letterLayoutOption}-${id}`;

  it('is absent on the résumé tab', () => {
    renderPanel({ activeOut: 'resume', coverOut: 'Dear Team, ...' });
    expect(screen.queryByTestId(letterOption('classic'))).not.toBeInTheDocument();
  });

  it('renders on the cover tab, defaulting to classic selected', () => {
    renderPanel({ activeOut: 'cover', resumeOut: '', coverOut: 'Dear Team, ...' });
    expect(screen.getByTestId(letterOption('classic'))).toHaveAttribute('aria-checked', 'true');
  });

  it('forwards a layout pick to onLetterLayoutChange, which drives the live preview + export', async () => {
    const user = userEvent.setup();
    const { onLetterLayoutChange } = renderPanel({
      activeOut: 'cover',
      resumeOut: '',
      coverOut: 'Dear Team, ...',
    });

    await user.click(screen.getByTestId(letterOption('refined')));
    expect(onLetterLayoutChange).toHaveBeenCalledWith('refined');
  });

  it('reflects an already-chosen layout as checked (preview + picker agree)', () => {
    renderPanel({
      activeOut: 'cover',
      resumeOut: '',
      coverOut: 'Dear Team, ...',
      letterLayoutId: 'banded',
    });
    expect(screen.getByTestId(letterOption('banded'))).toHaveAttribute('aria-checked', 'true');
  });

  it('a chosen layout reaches PdfPreview with the SAME value as the picker — export reads the same state', () => {
    // Mirrors the primary `target: 'both'` flow: the host re-renders with the
    // session-store value the picker just set, threaded straight to preview
    // (and to exportPDF/exportDOCX via the same host state — see export.test.ts).
    renderPanel({
      activeOut: 'cover',
      resumeOut: '',
      coverOut: 'Dear Team, ...',
      letterLayoutId: 'refined',
    });
    expect(screen.getByTestId(letterOption('refined'))).toHaveAttribute('aria-checked', 'true');
    expect(screen.getByTestId(TEST_IDS.documents.pdfPreview)).toHaveAttribute(
      'data-letter-layout-id',
      'refined'
    );
  });
});
