import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { OutputPanelDone } from './index';

// Heavy children are stubbed — this suite only covers the ATS view panel wiring.
vi.mock('@ajh/ui', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  useNotification: () => ({
    open: vi.fn(),
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
    destroy: vi.fn(),
  }),
}));
vi.mock('@/components/generation/PdfPreview', () => ({ PdfPreview: () => <div>PDF</div> }));
vi.mock('@/components/generation/EditableOutput', () => ({ EditableOutput: () => <div /> }));
vi.mock('@/components/generation/AtsViewPanel', () => ({
  AtsViewPanel: ({ path }: { path: string | null }) =>
    path ? <div data-testid="ats-panel">{path}</div> : null,
}));
vi.mock('@/services/use-contact-profile', () => ({
  useContactProfile: () => ({ data: undefined }),
}));
vi.mock('@/components/ui/ModelSelector', () => ({
  useSelectedModel: () => 'llama3',
  useSelectedProvider: () => 'ollama',
}));

function renderPanel(overrides: Partial<React.ComponentProps<typeof OutputPanelDone>> = {}) {
  render(
    <OutputPanelDone
      resumeOut="Led payments migration."
      coverOut="Dear team"
      activeOut="resume"
      meta={null}
      mode="ats"
      templateId="classic"
      atsMode={false}
      onActiveOutChange={vi.fn()}
      onCopy={vi.fn()}
      onExport={vi.fn()}
      onOutputChange={vi.fn()}
      onRegenerate={vi.fn()}
      copied={false}
      {...overrides}
    />
  );
}

describe('OutputPanelDone — ATS view panel', () => {
  it('shows the panel for a saved résumé export path', () => {
    renderPanel({ atsViewPath: '/out/cv.pdf' });
    expect(screen.getByTestId('ats-panel')).toHaveTextContent('/out/cv.pdf');
  });

  it('shows nothing without a path', () => {
    renderPanel();
    expect(screen.queryByTestId('ats-panel')).toBeNull();
  });

  it('hides the panel on the cover letter tab', () => {
    renderPanel({ activeOut: 'cover', atsViewPath: '/out/cv.pdf' });
    expect(screen.queryByTestId('ats-panel')).toBeNull();
  });
});
