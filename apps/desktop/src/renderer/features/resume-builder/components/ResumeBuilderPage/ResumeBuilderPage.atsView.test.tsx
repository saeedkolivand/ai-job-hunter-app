import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Button } from '@ajh/ui';

import { exportPDF, exportTXT } from '@/lib/generate';

import { ResumeBuilderPage } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/components/ui/ModelSelector', () => ({ ModelSelector: () => null }));
vi.mock('@/components/ui/AiSetupHint', () => ({ AiSetupHint: () => null }));
vi.mock('@/lib/generate', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  exportPDF: vi.fn(async () => '/out/cv.pdf'),
  exportDOCX: vi.fn(async () => '/out/cv.docx'),
  exportTXT: vi.fn(),
}));
vi.mock('../../hooks/useResumeBuilder', () => ({
  useResumeBuilder: () => ({
    language: 'en',
    locale: 'en',
    templateId: 'classic',
    atsMode: false,
    stage: 'done',
    output: '# Jane Doe',
    setResumeBuilder: vi.fn(),
    meta: {
      candidateName: 'Jane Doe',
      jobTitle: 'Engineer',
      companyName: 'Acme',
      resumeLanguage: 'en',
      jobAdLanguage: 'en',
      mismatch: false,
      targetLanguage: 'en',
      topRequirements: [],
    },
    canUseAI: true,
    aiReason: '',
    isComplete: true,
    isGenerating: false,
    streamBuffer: '',
    thinkingBuffer: '',
    modelLoading: false,
    tokenCount: 0,
    tokenStartMs: null,
    error: null,
    synthesize: vi.fn(),
    tailorToJob: vi.fn(),
    reset: vi.fn(),
  }),
}));
// Stub the heavy output view; it just drives `onExport` and renders the received path.
vi.mock('@/features/ai-generate/components/OutputPanelDone', () => ({
  OutputPanelDone: ({
    onExport,
    atsViewPath,
  }: {
    onExport: (f: 'pdf' | 'txt') => Promise<void>;
    atsViewPath?: string | null;
  }) => (
    <div>
      <Button onClick={() => void onExport('pdf')}>do-pdf</Button>
      <Button onClick={() => void onExport('txt')}>do-txt</Button>
      {atsViewPath && <div data-testid="ats-panel">{atsViewPath}</div>}
    </div>
  ),
}));

describe('ResumeBuilderPage — ATS view path', () => {
  it('is set after a PDF export and cleared when a TXT export starts', async () => {
    const user = userEvent.setup();
    render(<ResumeBuilderPage />);
    expect(screen.queryByTestId('ats-panel')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'do-pdf' }));
    expect(exportPDF).toHaveBeenCalled();
    expect(await screen.findByTestId('ats-panel')).toHaveTextContent('/out/cv.pdf');
    await user.click(screen.getByRole('button', { name: 'do-txt' }));
    expect(exportTXT).toHaveBeenCalled();
    expect(screen.queryByTestId('ats-panel')).toBeNull();
  });
});
