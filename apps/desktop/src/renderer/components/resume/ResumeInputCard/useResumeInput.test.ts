/**
 * useResumeInput — `onDocIdChange` propagation.
 *
 * Covers the two branches `useTailorPipeline` (the one real consumer) relies
 * on: selecting a saved résumé reports its id, and a manual edit clears a
 * PREVIOUSLY selected id back to `null` — the "doc-backed and unedited" rule
 * the apply flow's id-wins run request depends on. Everything else about this
 * hook (upload/import/profile-url flows) is exercised indirectly by
 * `ResumeInputCard.test.tsx` and is out of scope here.
 */
import { describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { DocumentRecord } from '@ajh/shared';

import { createMockClient, withProviders } from '@/test-support';

import { useResumeInput } from './useResumeInput';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
vi.mock('@ajh/ui', () => ({
  useNotification: () => ({ success: vi.fn(), error: vi.fn() }),
}));

const DOCS: DocumentRecord[] = [
  { id: 'doc-1', title: 'Résumé A', isDefault: true } as DocumentRecord,
];

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

// handleFileChange is the only path that touches the AppClient — unexercised
// here, so a bare mock client (every method a resolved-promise stub) is enough.
const wrapper = withProviders(createMockClient());

describe('useResumeInput — onDocIdChange', () => {
  it('reports the selected doc id when a saved résumé is loaded', () => {
    const onDocIdChange = vi.fn();
    const onChange = vi.fn();
    const { result } = renderHook(
      () => useResumeInput({ value: 'existing text', onChange, onDocIdChange }),
      { wrapper }
    );

    act(() => result.current.handleSelectSaved(DOCS[0] as DocumentRecord));

    expect(onDocIdChange).toHaveBeenCalledWith('doc-1');
    expect(result.current.selectedDocId).toBe('doc-1');
  });

  it('clears a previously selected doc id on a manual text edit', () => {
    const onDocIdChange = vi.fn();
    const onChange = vi.fn();
    const { result } = renderHook(
      () => useResumeInput({ value: 'existing text', onChange, onDocIdChange }),
      { wrapper }
    );

    act(() => result.current.handleSelectSaved(DOCS[0] as DocumentRecord));
    onDocIdChange.mockClear();

    act(() => result.current.handleTextChange('hand-edited text'));

    expect(onChange).toHaveBeenCalledWith('hand-edited text');
    expect(onDocIdChange).toHaveBeenCalledWith(null);
    expect(result.current.selectedDocId).toBeNull();
  });

  it('does not fire onDocIdChange on an edit when no doc was selected (no-op guard)', () => {
    const onDocIdChange = vi.fn();
    const onChange = vi.fn();
    const { result } = renderHook(() => useResumeInput({ value: '', onChange, onDocIdChange }), {
      wrapper,
    });

    act(() => result.current.handleTextChange('pasted text'));

    expect(onChange).toHaveBeenCalledWith('pasted text');
    expect(onDocIdChange).not.toHaveBeenCalled();
  });

  it('never throws when onDocIdChange is omitted', () => {
    const onChange = vi.fn();
    const { result } = renderHook(() => useResumeInput({ value: '', onChange }), { wrapper });

    expect(() =>
      act(() => result.current.handleSelectSaved(DOCS[0] as DocumentRecord))
    ).not.toThrow();
  });
});
