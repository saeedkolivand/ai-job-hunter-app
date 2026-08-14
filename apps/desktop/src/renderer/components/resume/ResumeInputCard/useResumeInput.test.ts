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

// `doc-1` is deliberately textless — it is also the auto-fill DEFAULT
// (`isDefault: true`), and several tests below rely on that effect staying
// inert (they pass `value: ''` to exercise an unrelated path). Giving it
// real text would auto-select it on every mount with an empty value,
// corrupting those tests' setup. `doc-with-text`/`doc-empty` (both
// non-default) carry the CR-1 fixtures instead, with no such side effect.
const DOCS: DocumentRecord[] = [
  { id: 'doc-1', title: 'Résumé A', isDefault: true } as DocumentRecord,
];

/** A doc that actually HAS text — use this wherever a test needs
 *  `handleSelectSaved` to genuinely succeed (`DOCS[0]`/`doc-1` deliberately
 *  does not, see above). */
const DOC_WITH_TEXT = {
  id: 'doc-with-text',
  title: 'Résumé B',
  isDefault: false,
} as DocumentRecord;

// `doc-empty` is the CR-1 fixture: present in the library (real id/title)
// but with no stored text — a metadata-only/blank document, which existing
// bare `{ _id, title, isDefault }` fixtures could never model on their own
// (their MISSING `text` key happened to mean the same thing, which is why
// this shape went unexercised).
vi.mock('@/services', () => ({
  useDocuments: () => ({
    data: [
      { _id: 'doc-1', title: 'Résumé A', isDefault: true },
      { _id: 'doc-with-text', title: 'Résumé B', isDefault: false, text: 'Résumé B full text' },
      { _id: 'doc-empty', title: 'Empty Doc', isDefault: false, text: '' },
    ],
  }),
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

    act(() => result.current.handleSelectSaved(DOC_WITH_TEXT));

    // The id notification and the text it's supposed to back must land
    // TOGETHER — see the CR-1 regression below for what happens when they don't.
    expect(onChange).toHaveBeenCalledWith('Résumé B full text');
    expect(onDocIdChange).toHaveBeenCalledWith('doc-with-text');
    expect(result.current.selectedDocId).toBe('doc-with-text');
  });

  it('clears a previously selected doc id on a manual text edit', () => {
    const onDocIdChange = vi.fn();
    const onChange = vi.fn();
    const { result } = renderHook(
      () => useResumeInput({ value: 'existing text', onChange, onDocIdChange }),
      { wrapper }
    );

    act(() => result.current.handleSelectSaved(DOC_WITH_TEXT));
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

describe('useResumeInput — handleSelectSaved on a metadata-only/empty doc (CR-1)', () => {
  // Bug: `selectDoc(doc.id)` fired unconditionally while `onChange` was
  // gated on `text` — selecting a doc with no stored text left the editor
  // showing whatever text was already there while `selectedDocId` claimed
  // it was backed by the empty doc. An id-first pipeline request would then
  // resolve server-side to the EMPTY stored text, silently discarding
  // whatever the user was actually looking at.
  const EMPTY_DOC = { id: 'doc-empty', title: 'Empty Doc', isDefault: false } as DocumentRecord;

  it('does not select (no onChange, no onDocIdChange) when the doc has no text', () => {
    const onDocIdChange = vi.fn();
    const onChange = vi.fn();
    const { result } = renderHook(
      () => useResumeInput({ value: 'existing text', onChange, onDocIdChange }),
      { wrapper }
    );

    act(() => result.current.handleSelectSaved(EMPTY_DOC));

    expect(onChange).not.toHaveBeenCalled();
    expect(onDocIdChange).not.toHaveBeenCalled();
    expect(result.current.selectedDocId).toBeNull();
  });

  it('does not clobber an ALREADY-selected doc id when selecting an empty doc', () => {
    const onDocIdChange = vi.fn();
    const onChange = vi.fn();
    const { result } = renderHook(
      () => useResumeInput({ value: 'existing text', onChange, onDocIdChange }),
      { wrapper }
    );

    act(() => result.current.handleSelectSaved(DOC_WITH_TEXT));
    onDocIdChange.mockClear();
    onChange.mockClear();

    act(() => result.current.handleSelectSaved(EMPTY_DOC));

    expect(onChange).not.toHaveBeenCalled();
    expect(onDocIdChange).not.toHaveBeenCalled();
    // Still 'doc-with-text' — the failed select of the empty doc must not
    // silently detach the id from the text that's genuinely still on screen.
    expect(result.current.selectedDocId).toBe('doc-with-text');
  });
});

describe('useResumeInput — remount regression (stale doc id after a hand-edit)', () => {
  // Repro: TailorWizard renders `{step === 1 && <StepResume />}`, so
  // ResumeInputCard — and this hook — REMOUNTS on every visit to the resume
  // step, resetting any component-local state. The surrounding form does
  // NOT reset, so `docId` here stands in for the host re-seeding this hook
  // from its own already-selected id (as `StepResume` does via
  // `getValues('resumeDocId')`) on that fresh mount.

  it('clears a host-seeded doc id (and notifies the host) on a post-remount hand edit', () => {
    const onDocIdChange = vi.fn();
    const onChange = vi.fn();
    const { result } = renderHook(
      () => useResumeInput({ value: 'existing text', onChange, docId: 'doc-1', onDocIdChange }),
      { wrapper }
    );

    // The remount itself must not lose the host's id — this is what makes
    // the edit below reachable at all.
    expect(result.current.selectedDocId).toBe('doc-1');

    act(() => result.current.handleTextChange('hand-edited text'));

    expect(onChange).toHaveBeenCalledWith('hand-edited text');
    expect(onDocIdChange).toHaveBeenCalledWith(null);
    expect(result.current.selectedDocId).toBeNull();
  });

  it('happy path: a doc-backed id survives a remount when the text is left unedited', () => {
    const onDocIdChange = vi.fn();
    const onChange = vi.fn();
    const { result } = renderHook(
      () => useResumeInput({ value: 'existing text', onChange, docId: 'doc-1', onDocIdChange }),
      { wrapper }
    );

    expect(result.current.selectedDocId).toBe('doc-1');
    expect(onDocIdChange).not.toHaveBeenCalled();
  });

  // handleRemove's sibling fix (same root cause as handleTextChange above,
  // fixed by the SAME `docId` seed) had no dedicated test of its own.
  it('handleRemove clears the seeded id/text when removing the doc that backs it', () => {
    const onDocIdChange = vi.fn();
    const onChange = vi.fn();
    const { result } = renderHook(
      () => useResumeInput({ value: 'existing text', onChange, docId: 'doc-1', onDocIdChange }),
      { wrapper }
    );

    act(() => result.current.handleRemove(DOCS[0] as DocumentRecord)); // DOCS[0].id === 'doc-1'

    expect(onDocIdChange).toHaveBeenCalledWith(null);
    expect(onChange).toHaveBeenCalledWith('');
    expect(result.current.selectedDocId).toBeNull();
  });

  it('handleRemove leaves the seeded id/text untouched when removing a DIFFERENT doc', () => {
    const onDocIdChange = vi.fn();
    const onChange = vi.fn();
    const { result } = renderHook(
      () => useResumeInput({ value: 'existing text', onChange, docId: 'doc-1', onDocIdChange }),
      { wrapper }
    );

    act(() =>
      result.current.handleRemove({
        id: 'doc-OTHER',
        title: 'Other',
        isDefault: false,
      } as DocumentRecord)
    );

    expect(onDocIdChange).not.toHaveBeenCalled();
    expect(onChange).not.toHaveBeenCalled();
    expect(result.current.selectedDocId).toBe('doc-1');
  });
});

describe('useResumeInput — activeDoc chip-label regression', () => {
  // Bug: `activeDoc` fell back to the DEFAULT saved doc whenever nothing was
  // explicitly selected, so the resting chip showed the default résumé's
  // title even when the on-screen text (a hand-edit/upload/paste/profile
  // import) had nothing to do with it. `selectedDocId === null` must mean
  // "no backing doc", never "assume the default".
  it('does not fall back to the default doc once selectedDocId is null', () => {
    const onChange = vi.fn();
    const { result } = renderHook(() => useResumeInput({ value: 'existing text', onChange }), {
      wrapper,
    });

    act(() => result.current.handleSelectSaved(DOC_WITH_TEXT));
    expect(result.current.activeDoc?.id).toBe('doc-with-text');

    // Any of the 3 unrelated call sites (upload/paste-save/profile-import)
    // clears selectedDocId the same way a hand-edit does — exercised here via
    // the already-covered text-change path, since the defect is in the
    // shared `activeDoc` derivation, not any one handler.
    act(() => result.current.handleTextChange('hand-edited text'));

    expect(result.current.selectedDocId).toBeNull();
    // `doc-1` is the DEFAULT saved doc (a different, unselected one) — must
    // NOT be shown as active just because it happens to be the default.
    expect(result.current.activeDoc).toBeUndefined();
  });
});
