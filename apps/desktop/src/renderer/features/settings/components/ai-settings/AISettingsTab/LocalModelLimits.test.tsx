/**
 * LocalModelLimits — the context-window COMMIT path.
 *
 * The slider writes renderer preferences on every tick and the backend row on
 * release. What matters is which releases reach the backend: a staged run reads
 * the backend row, so a skipped write is a run at the wrong `num_ctx` with the
 * right number on screen.
 *
 * The model-switch case is the regression guard: this component is not
 * remounted when the selected model changes (no `key` at the call site), so any
 * commit latch it keeps has to be keyed by model.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) =>
      params ? `${key} ${Object.values(params).join(' ')}` : key,
  }),
}));

const mockNotify = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return { ...actual, useNotification: () => mockNotify };
});

const save = vi.fn();
const inspectMutate = vi.fn();
/** Mutable so a case can simulate "the previous model was analyzed". */
const inspectState: { data: unknown; variables: { model: string } | undefined } = {
  data: undefined,
  variables: undefined,
};

vi.mock('@/services', () => ({
  useSaveProviderSettings: () => ({ save, isPending: false }),
  useInspectModel: () => ({
    mutate: inspectMutate,
    data: inspectState.data,
    variables: inspectState.variables,
    isPending: false,
    isSuccess: inspectState.data !== undefined,
  }),
  useSystemResources: () => ({
    resources: { freeRamGb: 16, hasGpu: false, freeVramGb: 0 },
  }),
}));

const setLocalModelLimits = vi.fn();

vi.mock('@/store/preferences-store', () => ({
  usePreferencesStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setLocalModelLimits, aiProviderConfig: undefined }),
}));

import { LocalModelLimits } from './LocalModelLimits';

const slider = () => screen.getAllByRole('slider')[0] as HTMLInputElement;

/** Move the slider and release it, the way a pointer drag ends. */
const dragTo = (value: number) => {
  fireEvent.change(slider(), { target: { value: String(value) } });
  fireEvent.pointerUp(slider(), { target: { value: String(value) } });
};

afterEach(() => {
  vi.clearAllMocks();
  inspectState.data = undefined;
  inspectState.variables = undefined;
});

describe('LocalModelLimits — committing the window', () => {
  it('writes the released value to the backend row for the selected model', () => {
    render(<LocalModelLimits selectedModel="model-a" />);

    dragTo(16_384);

    expect(save).toHaveBeenCalledWith(
      { provider: 'ollama', model: 'model-a', contextWindow: 16_384 },
      expect.anything()
    );
    // Renderer preferences are still written on the tick itself.
    expect(setLocalModelLimits).toHaveBeenCalledWith('model-a', { contextWindow: 16_384 });
  });

  it('does not re-send a release that moved nothing', () => {
    render(<LocalModelLimits selectedModel="model-a" />);

    dragTo(16_384);
    fireEvent.pointerUp(slider(), { target: { value: '16384' } });
    fireEvent.keyUp(slider(), { target: { value: '16384' } });

    expect(save).toHaveBeenCalledTimes(1);
  });

  it('REGRESSION: sends the same value again for a DIFFERENT model', () => {
    const { rerender } = render(<LocalModelLimits selectedModel="model-a" />);
    dragTo(16_384);

    // No `key` at the call site, so this is the same component instance with a
    // new prop — exactly what happens when the user picks another model.
    rerender(<LocalModelLimits selectedModel="model-b" />);
    dragTo(16_384);

    expect(save).toHaveBeenCalledTimes(2);
    expect(save).toHaveBeenLastCalledWith(
      { provider: 'ollama', model: 'model-b', contextWindow: 16_384 },
      expect.anything()
    );
  });

  it('surfaces a rejected write instead of leaving the slider lying', () => {
    save.mockImplementationOnce((_req, opts?: { onError?: (e: Error) => void }) =>
      opts?.onError?.(new Error('window out of range'))
    );
    render(<LocalModelLimits selectedModel="model-a" />);

    dragTo(16_384);

    expect(mockNotify.error).toHaveBeenCalledWith({
      message: 'settings.ai.localLimits.windowSaveFailed window out of range',
    });
  });
});

describe('LocalModelLimits — stale inspection data', () => {
  it('REGRESSION: does not clamp a new model with the previous model’s trained maximum', () => {
    // model-a was analyzed and reports a small window; the mutation result
    // survives the switch to model-b, which was never analyzed.
    inspectState.data = { contextLength: 4096 };
    inspectState.variables = { model: 'model-a' };

    const { rerender } = render(<LocalModelLimits selectedModel="model-a" />);
    expect(slider().max).toBe('4096');

    rerender(<LocalModelLimits selectedModel="model-b" />);

    // model-b has no measured maximum, so the schema ceiling applies — not
    // model-a's 4096, which would silently cap what gets committed for b.
    expect(slider().max).toBe('131072');
    dragTo(32_768);
    expect(save).toHaveBeenCalledWith(
      { provider: 'ollama', model: 'model-b', contextWindow: 32_768 },
      expect.anything()
    );
  });

  it('shows the measured maximum for the model it was measured on', () => {
    inspectState.data = { contextLength: 4096, parameterSize: '1.2B' };
    inspectState.variables = { model: 'model-a' };

    render(<LocalModelLimits selectedModel="model-a" />);

    expect(screen.getByText('1.2B')).toBeVisible();
  });
});
