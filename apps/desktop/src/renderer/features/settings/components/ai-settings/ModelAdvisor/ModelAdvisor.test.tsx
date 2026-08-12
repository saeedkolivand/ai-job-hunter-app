/**
 * ModelAdvisor — the guided flow, asserted per step by what that step SHOWS.
 *
 * The load-bearing claims: it is a real dialog, it names the stage a model
 * cannot fit, it never presents unmeasured as safe, and the server-level
 * section is advice the user runs — the wizard writes no server config and
 * makes no provider call of its own.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

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

const setStageOverrideAsync = vi.fn().mockResolvedValue({});

vi.mock('@/services', () => ({
  useModelInspections: () => ({
    byModel: {
      // A 4k-window model: below the documented input floor.
      'llama3.2:1b': { contextLength: 4096, parameterSize: '1.2B', quantization: 'Q4_K_M' },
      // Reported nothing — must read as NOT MEASURED, not as small.
      'mystery:latest': null,
    },
    isPending: false,
  }),
  useStageOverrides: () => ({ data: {} }),
  useSpendSummary: () => ({ data: { thinkingByModel: [] } }),
  useSetStageOverride: () => ({ mutateAsync: setStageOverrideAsync, isPending: false }),
}));

vi.mock('@/store/preferences-store', () => ({
  useAiProviderConfig: () => ({ providers: { ollama: { model: 'gpt-oss:20b', effort: 'high' } } }),
}));

import { ModelAdvisor } from './index';

const props = {
  open: true,
  onClose: vi.fn(),
  activeProvider: 'ollama',
  activeModel: 'gpt-oss:20b',
  installedModels: ['llama3.2:1b', 'mystery:latest'],
};

const next = async (user: ReturnType<typeof userEvent.setup>, times: number) => {
  for (let i = 0; i < times; i += 1) {
    await user.click(screen.getByText('settings.ai.advisor.next'));
  }
};

describe('ModelAdvisor — shell', () => {
  it('is a labelled dialog', () => {
    render(<ModelAdvisor {...props} />);

    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog).toHaveAccessibleName('settings.ai.advisor.title');
  });

  it('moves focus onto the step content when the step changes', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} />);

    await next(user, 1);

    // Focus follows the content that changed, not the button that changed it.
    expect(document.activeElement).toHaveAttribute('tabindex', '-1');
    expect(document.activeElement).toHaveTextContent('settings.ai.advisor.fit.intro');
  });
});

describe('ModelAdvisor — step 1, installed models', () => {
  it('reports the measured window and marks an undescribed model as not measured', () => {
    render(<ModelAdvisor {...props} />);

    expect(
      screen.getByText(/1\.2B · Q4_K_M · settings\.ai\.advisor\.models\.window/)
    ).toBeVisible();
    expect(screen.getByText('settings.ai.advisor.models.notMeasured')).toBeVisible();
  });
});

describe('ModelAdvisor — step 2, context fit', () => {
  it('names the stages a too-small window would cut', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} />);

    await next(user, 1);

    expect(screen.getByText('settings.ai.advisor.fit.verdict.too-small')).toBeVisible();
    expect(
      screen.getByText(/settings\.ai\.advisor\.fit\.overflow .*settings\.ai\.stages\.names\.draft/)
    ).toBeVisible();
  });

  it('says "not measured" for a model with no reported window', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} />);

    await next(user, 1);

    expect(screen.getByText('settings.ai.advisor.fit.verdict.unknown')).toBeVisible();
    expect(screen.getByText('settings.ai.advisor.fit.unknownHint')).toBeVisible();
  });
});

describe('ModelAdvisor — step 3, recommendations', () => {
  it('prints the server-level settings as commands to run, and writes nothing', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} />);

    await next(user, 2);

    expect(screen.getByText('OLLAMA_KV_CACHE_TYPE=q8_0')).toBeVisible();
    expect(screen.getByText('settings.ai.advisor.recommend.serverDisclaimer')).toBeVisible();
    // Advice only: reaching this step must not have written an override.
    expect(setStageOverrideAsync).not.toHaveBeenCalled();
  });
});

describe('ModelAdvisor — step 4, known-bad combinations', () => {
  it('flags a thinking-heavy local model at high effort', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} />);

    await next(user, 3);

    expect(screen.getByText(/settings\.ai\.advisor\.risk\.level\.known-bad-combo/)).toBeVisible();
    expect(screen.getByText(/settings\.ai\.advisor\.risk\.badCombo high/)).toBeVisible();
  });

  it('explains that local providers never report the split', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} />);

    await next(user, 3);

    // Absence of data is stated as absence of MEASUREMENT.
    expect(screen.getByText('settings.ai.advisor.risk.coverageNote')).toBeVisible();
  });
});
