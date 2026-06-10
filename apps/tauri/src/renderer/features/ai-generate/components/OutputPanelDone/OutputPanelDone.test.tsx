import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { OutputPanelDone } from './index';

// Stub the real-PDF preview (#24) — it renders the export via IPC, out of scope
// for this panel's preview/edit wiring test (covered in PdfPreview's own suite).
vi.mock('@/components/generation/PdfPreview', () => ({
  PdfPreview: () => <div data-testid="pdf-preview">PDF</div>,
}));

const RAW = 'Led **payments** migration at scale.';

function renderPanel(overrides: Partial<React.ComponentProps<typeof OutputPanelDone>> = {}) {
  const onOutputChange = vi.fn();
  const onExport = vi.fn();
  const onCopy = vi.fn();
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
      onCopy={onCopy}
      onExport={onExport}
      onOutputChange={onOutputChange}
      onRegenerate={vi.fn()}
      copied={false}
      {...overrides}
    />
  );
  return { onOutputChange, onExport, onCopy };
}

describe('OutputPanelDone — preview/edit', () => {
  it('shows the real-PDF preview by default (#24), not markdown or a textarea', () => {
    renderPanel();
    // The default Preview tab renders the real-PDF view, not the markdown fallback.
    expect(screen.getByTestId('pdf-preview')).toBeInTheDocument();
    expect(screen.queryByText(/\*\*payments\*\*/)).toBeNull();
    // No editable textarea while previewing.
    expect(screen.queryByRole('textbox')).toBeNull();
  });

  it('switches to a raw textarea with markers intact (export source untouched)', () => {
    const { onOutputChange } = renderPanel();
    // The Preview/Edit switch is a SegmentedControl radio group; the "Edit" radio's
    // accessible name resolves to "Edit" (or the raw i18n key) — both match.
    fireEvent.click(screen.getByRole('radio', { name: /edit/i }));

    const textarea = screen.getByRole<HTMLTextAreaElement>('textbox');
    // Raw text — including the **payments** markers the export pipeline reads.
    expect(textarea.value).toBe(RAW);
    // Switching views must not mutate the canonical output.
    expect(onOutputChange).not.toHaveBeenCalled();
  });
});
