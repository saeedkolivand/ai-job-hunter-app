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
    // The steps format numbers against the ACTIVE app language, not the OS one.
    i18n: { language: 'en' },
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
      // A 4k-window model: below the documented input floor. Its measured size
      // also makes it a legitimate suggestion against the 32.8B active model.
      'llama3.2:1b': {
        contextLength: 4096,
        parameterSize: '1.2B',
        quantization: 'Q4_K_M',
        family: 'llama',
      },
      'qwen3:32b': { contextLength: 40_960, parameterSize: '32.8B', family: 'qwen3' },
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

  it('keeps focus inside the dialog when the step changes', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} />);

    await next(user, 1);

    // Focus stays on the real control the user pressed. It used to be moved to
    // the step container, which — as a `tabindex="-1"` node — sat outside the
    // focus trap's FOCUSABLE query, so Shift+Tab escaped to the page behind.
    const dialog = screen.getByRole('dialog');
    expect(document.activeElement).toBe(screen.getByText('settings.ai.advisor.next'));
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  it('announces the new step instead of stealing focus for it', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} />);

    const heading = screen.getByText(/settings\.ai\.advisor\.stepCounter 1 4/);
    expect(heading).toHaveAttribute('aria-live', 'polite');

    await next(user, 1);

    // The same live region now carries step 2 — that is the announcement.
    expect(screen.getByText(/settings\.ai\.advisor\.stepCounter 2 4/)).toHaveAttribute(
      'aria-live',
      'polite'
    );
  });

  it('names the step region with the step heading', () => {
    render(<ModelAdvisor {...props} />);

    const region = screen.getByRole('group');
    const heading = screen.getByText(/settings\.ai\.advisor\.stepCounter 1 4/);
    expect(region).toHaveAttribute('aria-labelledby', heading.id);
  });

  it('renders the step counter from the step list, not from the copy', () => {
    render(<ModelAdvisor {...props} />);

    expect(screen.getByText(/settings\.ai\.advisor\.stepCounter 1 4/)).toBeVisible();
  });

  it('can be closed from any step, not only the last', async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} onClose={onClose} />);

    await user.click(screen.getByLabelText('settings.ai.advisor.close'));

    expect(onClose).toHaveBeenCalled();
  });
});

describe('ModelAdvisor — a cloud-only machine (no local models)', () => {
  it('says there is nothing to measure instead of showing an empty list', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} installedModels={[]} />);

    expect(screen.getByText('settings.ai.advisor.models.emptyTitle')).toBeVisible();

    await next(user, 1);
    expect(screen.getByText('settings.ai.advisor.fit.emptyTitle')).toBeVisible();
  });

  it('says there is no model to assess when nothing is selected or pinned', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...props} installedModels={[]} activeModel={undefined} />);

    await next(user, 3);

    expect(screen.getByText('settings.ai.advisor.risk.emptyTitle')).toBeVisible();
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

/**
 * The advisor renders the settings card's suggestion banner. Both of its
 * buttons unmount themselves, and this one lives inside a focus-trapped dialog:
 * dropping focus on <body> lets the very next Tab reach the page behind the
 * overlay, because the trap's keydown listener is bound to the panel.
 */
describe('ModelAdvisor — step 3, the suggestion inside the dialog', () => {
  const withSuggestion = {
    ...props,
    activeModel: 'qwen3:32b',
    installedModels: ['qwen3:32b', 'llama3.2:1b'],
  };

  it('keeps focus inside the dialog after the suggestion is declined', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...withSuggestion} />);
    await next(user, 2);

    await user.click(screen.getByText('settings.ai.stages.suggest.dismiss'));

    const dialog = screen.getByRole('dialog');
    expect(dialog.contains(document.activeElement)).toBe(true);
    expect(document.activeElement).toBe(screen.getByText('settings.ai.advisor.next'));
  });

  it('replaces the declined offer with an explanation, not with nothing', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...withSuggestion} />);
    await next(user, 2);

    await user.click(screen.getByText('settings.ai.stages.suggest.dismiss'));

    // The section used to go blank: dismissal lived inside the banner, so the
    // parent's "is there a suggestion" check still said yes and rendered
    // neither the offer nor its fallback.
    expect(screen.getByText('settings.ai.advisor.recommend.stagesDeclined')).toBeVisible();
    expect(screen.queryByText('settings.ai.stages.suggest.apply')).toBeNull();
  });

  it('keeps focus inside the dialog after the suggestion is accepted', async () => {
    const user = userEvent.setup();
    render(<ModelAdvisor {...withSuggestion} />);
    await next(user, 2);

    await user.click(screen.getByText('settings.ai.stages.suggest.apply'));

    expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true);
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
