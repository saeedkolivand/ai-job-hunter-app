/** Rule 16: an in-flight AI Generate run survives leaving the page and coming back. */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as Ui from '@ajh/ui';

import type * as Gen from '@/lib/generate';
import { resetAIGenerateAll, useAIGenerateRunStore, useSessionStore } from '@/store/session-store';

import { AIGeneratePage } from './index';

const gen = vi.hoisted(() => ({
  onToken: (_t: string) => {},
  finish: (_t: string) => {},
  signal: undefined as AbortSignal | undefined,
}));

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
vi.mock('@ajh/ui', async (orig) => ({
  ...(await orig<typeof Ui>()),
  useNotification: () => ({ success: vi.fn(), error: vi.fn() }),
}));
vi.mock('@/components/contact/ContactPromptModal', () => ({ ContactPromptModal: () => null }));
vi.mock('@/components/layout/PageTransition', () => ({
  PageTransition: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));
vi.mock('@/components/ui/ModelSelector', () => ({
  useCanUseAI: () => ({ canUse: true }),
  useSelectedModel: () => 'm',
}));
vi.mock('@/hooks/use-quality-recheck', () => ({
  useQualityRecheck: () => ({ recheck: vi.fn(), rechecking: false }),
}));
vi.mock('@/hooks/use-research-company-default', () => ({
  useResearchCompanyDefault: () => [false, vi.fn()],
}));
vi.mock('@/services', () => ({ useExtractText: () => ({}) }));
vi.mock('@/services/use-ai-generations', () => ({
  useSaveAiGeneration: () => ({ mutate: vi.fn() }),
}));
vi.mock('@/features/ai-generate/hooks/useFileUpload', () => ({
  useFileUpload: () => ({ handleUpload: vi.fn() }),
}));
vi.mock('./useContactPromptGate', () => ({
  useContactPromptGate: (g: () => void) => ({
    contactModalOpen: false,
    closeContactModal: vi.fn(),
    requestGenerate: g,
    continueFromContactPrompt: vi.fn(),
  }),
}));
vi.mock('@/lib/generate/provider-context', () => ({
  resolveActiveProvider: () => ({ activeProvider: 'ollama', activeModel: 'm' }),
}));
vi.mock('@/lib/generate', async (orig) => ({
  ...(await orig<typeof Gen>()),
  computeQualityReport: async () => null,
  generateResume: (...a: unknown[]) => {
    gen.onToken = a[5] as (t: string) => void;
    gen.signal = a[7] as AbortSignal;
    return new Promise<string>((res, rej) => {
      gen.finish = res;
      gen.signal?.addEventListener('abort', () => rej(new Error('aborted')));
    });
  },
}));

vi.mock('@/features/ai-generate/components/LeftPanel', () => ({
  LeftPanel: ({ onReset }: { onReset: () => void }) => <button onClick={onReset}>cancel</button>,
}));
vi.mock('@/features/ai-generate/components/GenerateWizard', () => ({
  GenerateWizard: ({ onGenerate }: { onGenerate: () => void }) => (
    <button onClick={onGenerate}>go</button>
  ),
}));
vi.mock('@/features/ai-generate/components/OutputPanelGenerating', () => ({
  OutputPanelGenerating: ({
    streamBuffer,
    onCancel,
  }: {
    streamBuffer: string;
    onCancel?: () => void;
  }) => (
    <div>
      generating:{streamBuffer}
      <button onClick={onCancel}>stop</button>
    </div>
  ),
}));
vi.mock('@/features/ai-generate/components/OutputPanelDone', () => ({
  OutputPanelDone: ({ resumeOut }: { resumeOut: string }) => <div>done:{resumeOut}</div>,
}));
vi.mock('@/features/ai-generate/components/OutputPanelIdle', () => ({
  OutputPanelIdle: () => null,
}));
vi.mock('@/features/ai-generate/components/OutputPanelExtracting', () => ({
  OutputPanelExtracting: () => null,
}));

beforeEach(() => {
  resetAIGenerateAll();
  useSessionStore.getState().setAIGenerate({
    stage: 'configuring',
    target: 'resume',
    resume: 'r'.repeat(60),
    jobAd: 'j'.repeat(60),
    meta: {
      resumeLanguage: 'en',
      jobAdLanguage: 'en',
      mismatch: false,
      candidateName: 'A',
      jobTitle: 'T',
      companyName: 'C',
      targetLanguage: 'en',
      topRequirements: [],
    },
  });
});

async function startAndLeave() {
  const first = render(<AIGeneratePage />);
  await userEvent.click(screen.getByText('go'));
  act(() => gen.onToken('tok'));
  expect(screen.getByText('generating:tok')).toBeTruthy();
  first.unmount();
}

describe('AIGeneratePage — survives navigation', () => {
  it('shows the run still in flight on remount, and Cancel aborts it', async () => {
    await startAndLeave();
    expect(gen.signal?.aborted).toBe(false);
    act(() => gen.onToken('en'));

    render(<AIGeneratePage />);
    expect(screen.getByText('generating:token')).toBeTruthy();
    expect(useAIGenerateRunStore.getState().isGenerating).toBe(true);

    await userEvent.click(screen.getByText('cancel'));
    expect(gen.signal?.aborted).toBe(true);
    expect(useAIGenerateRunStore.getState().isGenerating).toBe(false);
  });

  it('the Cancel control aborts the live run without an error and returns to configuring (#1412)', async () => {
    await startAndLeave();
    render(<AIGeneratePage />);

    await userEvent.click(screen.getByText('stop'));

    expect(gen.signal?.aborted).toBe(true);
    await vi.waitFor(() => expect(useAIGenerateRunStore.getState().isGenerating).toBe(false));
    expect(useAIGenerateRunStore.getState().error).toBeNull();
    expect(useSessionStore.getState().aiGenerate.stage).toBe('configuring');
  });

  it('shows the result on remount when the run finished while away', async () => {
    await startAndLeave();
    await act(async () => gen.finish('final resume'));

    render(<AIGeneratePage />);
    expect(screen.getByText('done:final resume')).toBeTruthy();
  });
});
