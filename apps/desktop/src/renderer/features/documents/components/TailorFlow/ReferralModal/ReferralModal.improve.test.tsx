/**
 * ReferralModal — the "Improve with AI" affordance: presets, free-text instruction,
 * and its keyboard behaviour. Shared stubs live in `modal.test-support.tsx`.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, screen } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

import { fillPersonName, renderModal } from './modal.test-helpers';
import { mockImprove, resetStub, stub } from './modal.test-support';

vi.mock('@/services', async () => (await import('./modal.test-support')).servicesModule);
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
vi.mock('@/components/ui/ModelSelector', async () => {
  return (await import('./modal.test-support')).modelSelectorModule;
});
vi.mock(
  './useReferralDraft',
  async () => (await import('./modal.test-support')).referralDraftModule
);
vi.mock('./ReferralList', async () => (await import('./modal.test-support')).referralListModule);
vi.mock('@ajh/ui', async (importOriginal) => {
  return (await import('./modal.test-support')).uiModule(await importOriginal<typeof AjhUi>());
});

beforeEach(resetStub);

afterEach(() => {
  vi.clearAllMocks();
});

describe('ReferralModal — Improve with AI affordance', () => {
  it('is NOT rendered when draft is empty', () => {
    // stub.draft = '' (reset in beforeEach)
    renderModal();
    fillPersonName('Bob Chen');

    // The improve section is conditionally rendered only when gen.draft is non-empty.
    expect(
      screen.queryByRole('button', { name: /autopilot\.referral\.improvePresets\.warmer/i })
    ).toBeNull();
    expect(screen.queryByLabelText('autopilot.referral.improveInstruction')).toBeNull();
  });

  it('IS rendered when a draft exists and not generating', () => {
    stub.draft = 'Hi Bob, I wanted to reach out.';
    renderModal();
    fillPersonName('Bob Chen');

    // All four preset chips must be present.
    expect(
      screen.getByRole('button', { name: 'autopilot.referral.improvePresets.warmer' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'autopilot.referral.improvePresets.shorter' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'autopilot.referral.improvePresets.moreSpecific' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'autopilot.referral.improvePresets.fixGrammar' })
    ).toBeInTheDocument();

    // Free-text input and Apply button must be present.
    expect(screen.getByLabelText('autopilot.referral.improveInstruction')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'autopilot.referral.improveApply' })
    ).toBeInTheDocument();
  });

  it('clicking a preset chip calls gen.improve with the preset i18n label', () => {
    stub.draft = 'Hi Bob, I wanted to reach out.';
    renderModal();
    fillPersonName('Bob Chen');

    act(() => {
      fireEvent.click(
        screen.getByRole('button', { name: 'autopilot.referral.improvePresets.warmer' })
      );
    });

    // The mock t() returns the key itself, so the instruction passed is the full key.
    expect(mockImprove).toHaveBeenCalledTimes(1);
    expect(mockImprove).toHaveBeenCalledWith('autopilot.referral.improvePresets.warmer');
  });

  it('submitting a custom instruction via Apply button calls gen.improve', () => {
    stub.draft = 'Hi Bob, I wanted to reach out.';
    renderModal();
    fillPersonName('Bob Chen');

    const instructionInput = screen.getByLabelText('autopilot.referral.improveInstruction');
    fireEvent.change(instructionInput, { target: { value: 'mention the Kafka work' } });

    act(() => {
      fireEvent.click(screen.getByRole('button', { name: 'autopilot.referral.improveApply' }));
    });

    expect(mockImprove).toHaveBeenCalledTimes(1);
    expect(mockImprove).toHaveBeenCalledWith('mention the Kafka work');
  });

  it('Apply button is disabled when the instruction input is blank', () => {
    stub.draft = 'Hi Bob, I wanted to reach out.';
    renderModal();
    fillPersonName('Bob Chen');

    const applyBtn = screen.getByRole('button', { name: 'autopilot.referral.improveApply' });
    // Instruction is empty (default state) → Apply must be disabled.
    expect(applyBtn).toBeDisabled();
  });

  // ── Keyboard behaviour on the improve instruction input ───────────────────────

  it('pressing Enter on the instruction input submits the instruction (calls improve)', () => {
    stub.draft = 'Hi Bob, I wanted to reach out.';
    renderModal();
    fillPersonName('Bob Chen');

    const instructionInput = screen.getByLabelText('autopilot.referral.improveInstruction');
    fireEvent.change(instructionInput, { target: { value: 'be more concise' } });

    act(() => {
      fireEvent.keyDown(instructionInput, { key: 'Enter', code: 'Enter', shiftKey: false });
    });

    expect(mockImprove).toHaveBeenCalledTimes(1);
    expect(mockImprove).toHaveBeenCalledWith('be more concise');
  });

  it('pressing Shift+Enter on the instruction input does NOT submit', () => {
    stub.draft = 'Hi Bob, I wanted to reach out.';
    renderModal();
    fillPersonName('Bob Chen');

    const instructionInput = screen.getByLabelText('autopilot.referral.improveInstruction');
    fireEvent.change(instructionInput, { target: { value: 'be more concise' } });

    act(() => {
      // Shift+Enter must not trigger improve (allows line breaks in a future multi-line variant).
      fireEvent.keyDown(instructionInput, { key: 'Enter', code: 'Enter', shiftKey: true });
    });

    expect(mockImprove).not.toHaveBeenCalled();
  });
});
