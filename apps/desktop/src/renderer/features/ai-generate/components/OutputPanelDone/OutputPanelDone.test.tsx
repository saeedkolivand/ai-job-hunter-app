import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';

import type { LetterLayoutId } from '@/lib/generate';

import { OutputPanelDone } from './index';

// OutputPanelDone always mounts ExportModal (gated by its own `open` prop, not a
// conditional render), and ExportModal now calls useNotification() to surface a
// rejected export — stub it so this panel-wiring suite doesn't need a real
// NotificationProvider tree.
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useNotification: () => ({
      open: vi.fn(),
      success: vi.fn(),
      error: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
      destroy: vi.fn(),
    }),
  };
});

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

// EditableOutput also reads useSelectedModel, now backed by the `useActiveConfig`
// query (task #16) which reaches for AppClientProvider — stub it to a plain model.
vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'llama3',
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

  // ── ATS toggle beside the picker ────────────────────────────────────────────
  // This panel REPLACES the wizard once a run is done, so a layout picked here
  // must be undoable here: the letter renderer reads `atsMode` (`data.opts.ats`)
  // and a decorated layout has no other off switch on this screen.

  const coverProps = { activeOut: 'cover' as const, resumeOut: '', coverOut: 'Dear Team, ...' };

  it.each(['banded', 'sidebar', 'monogram'] as const)(
    'shows the ATS toggle on the cover tab for the decorated layout %s',
    (letterLayoutId) => {
      renderPanel({ ...coverProps, letterLayoutId, onAtsModeChange: vi.fn() });
      expect(screen.getByRole('switch')).toBeInTheDocument();
      expect(screen.getByText('ATS-safe mode')).toBeInTheDocument();
    }
  );

  it.each(['classic', 'refined', 'navy'] as const)(
    'hides the ATS toggle for the undecorated layout %s (it would change nothing)',
    (letterLayoutId) => {
      renderPanel({ ...coverProps, letterLayoutId, onAtsModeChange: vi.fn() });
      expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    }
  );

  it('hides the ATS toggle on the résumé tab even when the letter is decorated', () => {
    renderPanel({
      activeOut: 'resume',
      coverOut: 'Dear Team, ...',
      letterLayoutId: 'monogram',
      onAtsModeChange: vi.fn(),
    });
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it('hides the ATS toggle when the host owns no atsMode setter', () => {
    renderPanel({ ...coverProps, letterLayoutId: 'monogram' });
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  });

  it('flips atsMode on — the off switch for the monogram tile after generation', async () => {
    const user = userEvent.setup();
    const onAtsModeChange = vi.fn();
    renderPanel({ ...coverProps, letterLayoutId: 'monogram', onAtsModeChange });

    await user.click(screen.getByRole('switch'));

    expect(onAtsModeChange).toHaveBeenCalledWith(true);
  });

  it('flips atsMode back off and shows the current state via aria-checked', async () => {
    const user = userEvent.setup();
    const onAtsModeChange = vi.fn();
    renderPanel({ ...coverProps, letterLayoutId: 'monogram', atsMode: true, onAtsModeChange });

    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
    await user.click(screen.getByRole('switch'));
    expect(onAtsModeChange).toHaveBeenCalledWith(false);
  });

  // Real host state, so the announcement is driven by the actual user action
  // (picking a layout) rather than a prop swap.
  function StatefulCoverPanel() {
    const [letterLayoutId, setLetterLayoutId] = useState<LetterLayoutId | undefined>(undefined);
    return (
      <OutputPanelDone
        resumeOut=""
        coverOut="Dear Team, ..."
        activeOut="cover"
        meta={null}
        mode="ats"
        templateId="classic"
        atsMode={false}
        letterLayoutId={letterLayoutId}
        onActiveOutChange={vi.fn()}
        onLetterLayoutChange={setLetterLayoutId}
        onAtsModeChange={vi.fn()}
        onCopy={vi.fn()}
        onExport={vi.fn()}
        onOutputChange={vi.fn()}
        onRegenerate={vi.fn()}
        copied={false}
      />
    );
  }

  it('announces the toggle becoming available from an always-mounted live region', async () => {
    // The region has to exist BEFORE the toggle appears — a live region does not
    // announce its own first render, so "empty while hidden" is the load-bearing
    // half of this pair.
    const user = userEvent.setup();
    render(<StatefulCoverPanel />);
    expect(screen.getByRole('status')).toHaveTextContent('');

    await user.click(screen.getByTestId(letterOption('monogram')));

    expect(screen.getByRole('switch')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('ATS-safe mode is available');
  });

  // Same symmetry as the other two surfaces: leaving a decorated layout releases
  // the shared flag, so a re-picked Monogram is not silently already-ATS'd.
  it('releases atsMode when the layout drops to classic and nothing else reads it', async () => {
    const user = userEvent.setup();
    const onAtsModeChange = vi.fn();
    renderPanel({
      ...coverProps,
      templateId: 'classic',
      letterLayoutId: 'monogram',
      atsMode: true,
      onAtsModeChange,
    });

    await user.click(screen.getByTestId(letterOption('classic')));

    expect(onAtsModeChange).toHaveBeenCalledWith(false);
  });

  it('keeps atsMode on that change when a design-tier résumé template still reads it', async () => {
    const user = userEvent.setup();
    const onAtsModeChange = vi.fn();
    renderPanel({
      ...coverProps,
      templateId: 'atelier',
      letterLayoutId: 'monogram',
      atsMode: true,
      onAtsModeChange,
    });

    await user.click(screen.getByTestId(letterOption('classic')));

    expect(onAtsModeChange).not.toHaveBeenCalled();
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

// The re-check BEHAVIOUR (ownership epoch, merge-onto-live-base, persistence)
// lives in `useQualityRecheck`, which the HOST owns — see
// `hooks/use-quality-recheck.test.ts`. This panel is unmounted the moment a
// Regenerate switches the stage, so it must not own that state; all it owes is
// forwarding the host's action into the quality panel.
describe('OutputPanelDone — quality panel Re-check wiring', () => {
  const STALE_REPORT = {
    schemaVersion: 2 as const,
    pipeline: 'fast' as const,
    generatedAt: 1,
    resume: {
      report: {
        ok: true,
        issues: [],
        metrics: {
          keywordCoverage: null,
          topRequirementHits: 0,
          duplicateRatio: 0,
          rolesSource: 0,
          rolesOutput: 0,
        },
      },
      // Mismatches RAW's hash on purpose — the badge renders stale, so the
      // panel's Re-check button is reachable.
      sourceTextHash: -1,
    },
  };

  it('forwards the panel Re-check action to the host that owns it', async () => {
    const onRecheck = vi.fn();
    const user = userEvent.setup();

    renderPanel({ report: STALE_REPORT, onRecheck });

    await user.click(screen.getByRole('button', { name: /checked before your edits/i }));
    await user.click(screen.getByRole('button', { name: /re-check/i }));

    expect(onRecheck).toHaveBeenCalledTimes(1);
  });

  it('hides the Re-check action when the host provides none', async () => {
    const user = userEvent.setup();
    renderPanel({ report: STALE_REPORT });

    await user.click(screen.getByRole('button', { name: /checked before your edits/i }));
    expect(screen.queryByRole('button', { name: /re-check/i })).toBeNull();
  });
});
