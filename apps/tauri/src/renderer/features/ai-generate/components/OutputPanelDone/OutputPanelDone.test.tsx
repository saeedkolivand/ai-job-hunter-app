import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { OutputPanelDone } from './index';

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
  it('renders prettified markdown by default, hiding the raw ** emphasis markers', () => {
    renderPanel();
    // The **keyword** marker renders as bold, not as literal asterisks.
    expect(screen.getByText('payments').tagName).toBe('STRONG');
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
