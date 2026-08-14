/**
 * StepResume — remount regression for the "stale doc id after a hand-edit"
 * bug: `TailorWizard` renders `{step === 1 && <StepResume />}`, so
 * `ResumeInputCard` (and its `useResumeInput` hook) remounts every time the
 * wizard revisits this step, while the surrounding RHF form does not. This
 * exercises the real seam that broke: the same `useForm()` instance across
 * an unmount/remount, confirming a post-remount hand edit clears the form's
 * `resumeDocId` (not just the hook's own local state, which
 * `useResumeInput.test.ts` already covers).
 */
import { FormProvider, useForm, type UseFormReturn } from 'react-hook-form';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, renderHook, screen } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

import { createMockClient, withProviders } from '@/test-support';

import { buildTailorDefaults, type TailorWizardState } from '../../lib/tailor-state';
import { StepResume } from './index';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

vi.mock('@ajh/ui', async () => {
  const actual = await vi.importActual<typeof AjhUi>('@ajh/ui');
  return { ...actual, useNotification: () => ({ success: vi.fn(), error: vi.fn() }) };
});

vi.mock('@/services', () => ({
  useDocuments: () => ({ data: [{ _id: 'doc-1', title: 'Résumé A', isDefault: true }] }),
  useProfileImport: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useRemoveDocument: () => ({ mutateAsync: vi.fn() }),
  useSetDefaultDocument: () => ({ mutateAsync: vi.fn() }),
}));

vi.mock('@/hooks/use-import-with-ocr', () => ({
  useImportWithOcr: () => ({
    importFile: vi.fn(),
    isOcr: false,
    isPending: false,
    review: null,
    clearReview: vi.fn(),
  }),
}));

const clientWrapper = withProviders(createMockClient());

function Harness({ methods }: { methods: UseFormReturn<TailorWizardState> }) {
  return (
    <FormProvider {...methods}>
      <StepResume />
    </FormProvider>
  );
}

const renderStep = (methods: UseFormReturn<TailorWizardState>) =>
  render(<Harness methods={methods} />, { wrapper: clientWrapper });

describe('StepResume — remount + hand edit', () => {
  it('clears the form doc id on a hand edit after the step remounts', () => {
    // One `useForm()` instance shared across both renders below — this is
    // the part of the repro a hook-only test can't show: the wizard's form
    // survives the step swap even though the step component itself doesn't.
    const { result: methods } = renderHook(() =>
      useForm<TailorWizardState>({
        defaultValues: { ...buildTailorDefaults('existing text'), resumeDocId: 'doc-1' },
      })
    );

    const { unmount } = renderStep(methods.current);
    unmount(); // navigate away from the resume step
    renderStep(methods.current); // navigate back — ResumeInputCard remounts

    fireEvent.click(screen.getByRole('button', { name: /resumeInput\.change/i }));
    fireEvent.change(screen.getByPlaceholderText('resumeInput.placeholder'), {
      target: { value: 'hand-edited text' },
    });

    expect(methods.current.getValues('resumeDocId')).toBeUndefined();
    expect(methods.current.getValues('resume')).toBe('hand-edited text');
  });

  it('happy path: the form doc id survives a remount when the text is left unedited', () => {
    const { result: methods } = renderHook(() =>
      useForm<TailorWizardState>({
        defaultValues: { ...buildTailorDefaults('existing text'), resumeDocId: 'doc-1' },
      })
    );

    const { unmount } = renderStep(methods.current);
    unmount();
    renderStep(methods.current);

    expect(methods.current.getValues('resumeDocId')).toBe('doc-1');
  });
});
