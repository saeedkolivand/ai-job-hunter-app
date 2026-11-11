/** CreationWizard — the no-resume warning on a create/update reply is shown, localized. */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import { buildDefaults } from '@/features/autopilot/lib/wizard-state';
import { useSessionStore } from '@/store/session-store';

import { CreationWizard } from './index';

const notify = { warning: vi.fn(), error: vi.fn(), success: vi.fn(), info: vi.fn() };
vi.mock('@ajh/ui', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  useNotification: () => notify,
}));
vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

const create = vi.fn();
const update = vi.fn();
vi.mock('@/services', () => ({
  useCreateAutopilot: () => ({ mutateAsync: create, isPending: false }),
  useUpdateAutopilot: () => ({ mutateAsync: update, isPending: false }),
  useJobPreferences: () => ({ data: undefined }),
}));
vi.mock('@/features/autopilot/components/wizard-steps/StepTarget', () => ({
  StepTarget: () => null,
}));
vi.mock('@/features/autopilot/components/wizard-steps/StepFilter', () => ({
  StepFilter: () => null,
}));
vi.mock('@/features/autopilot/components/wizard-steps/StepAction', () => ({
  StepAction: () => null,
}));
vi.mock('@/features/autopilot/components/wizard-steps/StepSchedule', () => ({
  StepSchedule: () => null,
}));

const seed = (editingId: string | null) =>
  useSessionStore.setState((s) => ({
    autopilot: {
      ...s.autopilot,
      wizardStep: 3,
      editingId,
      wizardForm: { ...buildDefaults(undefined), name: 'Jane search', boards: ['a'], query: 'dev' },
    },
  }));

async function submit(button: string, onDone = vi.fn()) {
  render(<CreationWizard onDone={onDone} onCancel={vi.fn()} />);
  fireEvent.click(screen.getByText(button));
  await waitFor(() => expect(onDone).toHaveBeenCalled());
}

beforeEach(() => vi.clearAllMocks());

describe('CreationWizard — no-resume warning', () => {
  it('shows the localized warning after create, not the raw backend text', async () => {
    seed(null);
    create.mockResolvedValue({ _id: 'a1', warning: 'raw English' });
    await submit('autopilot.wizard.create');
    expect(notify.warning).toHaveBeenCalledWith({ message: 'autopilot.wizard.noResumeWarning' });
  });

  it('shows it after edit too', async () => {
    seed('a1');
    update.mockResolvedValue({ _id: 'a1', warning: 'raw English' });
    await submit('autopilot.wizard.save');
    expect(notify.warning).toHaveBeenCalledWith({ message: 'autopilot.wizard.noResumeWarning' });
  });

  it('stays silent without a warning', async () => {
    seed(null);
    create.mockResolvedValue({ _id: 'a1' });
    await submit('autopilot.wizard.create');
    expect(notify.warning).not.toHaveBeenCalled();
  });

  it('reports an update of an unknown id (null) as a failure, not a save', async () => {
    seed('gone');
    update.mockResolvedValue(null);
    const onDone = vi.fn();
    render(<CreationWizard onDone={onDone} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByText('autopilot.wizard.save'));
    await screen.findByText('autopilot.wizard.updateFailed');
    expect(onDone).not.toHaveBeenCalled();
  });
});
