/**
 * Shared stubs + helpers for the ReferralModal tests (`ReferralModal.test.tsx`,
 * `ReferralModal.improve.test.tsx`).
 *
 * Heavy UI pieces (ModelSelector, ModalShell focus trap, ReferralList, the
 * streaming draft hook) are replaced with lightweight stubs so the tests stay
 * fast and deterministic. The real component logic (overLimit computation,
 * canSave guard, payload mapping) is still exercised through the real
 * ReferralModal code. `vi.mock` is hoisted per file, so each test file keeps its
 * own one-line `vi.mock(..., async () => (await import(...)).x)` calls. This module is
 * what those factories import, so it must NEVER import the component under test (a value
 * import would deadlock the factory) — the render helpers live in `modal.test-helpers.tsx`.
 */
import { vi } from 'vitest';

import type { ReferralContact, ReferralUpsertRequest } from '@ajh/shared/ipc';
import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';

export const mockUpsertMutate =
  vi.fn<(req: ReferralUpsertRequest, opts?: { onSuccess?: () => void }) => void>();
export const mockGenerate = vi.fn<() => Promise<void>>(async () => {});
export const mockImprove = vi.fn<(instruction: string) => Promise<void>>(async () => {});

/** Per-test inputs the stubs read: the draft the hook "produced", and an
 *  optional pre-seeded contact (exercises the dedup branch in save()). */
export const stub: { draft: string; contacts: ReferralContact[] } = { draft: '', contacts: [] };

export function resetStub() {
  stub.draft = '';
  stub.contacts = [];
  mockUpsertMutate.mockClear();
  mockGenerate.mockClear();
  mockImprove.mockClear();
}

export const servicesModule: Record<string, unknown> = {
  useReferrals: () => ({ data: stub.contacts }),
  useUpsertReferral: () => ({ mutate: mockUpsertMutate, isPending: false }),
  // useRemoveReferral is consumed by ReferralList (which is stubbed), so it only
  // needs to exist for the service barrel import to resolve.
  useRemoveReferral: () => ({ mutate: vi.fn(), isPending: false }),
};

// Stub the whole ModelSelector component so we don't need the full provider tree.
export const modelSelectorModule = {
  ModelSelector: () => null,
  useCanUseAI: () => ({ canUse: true, reason: null }),
  useSelectedModel: () => 'llama3',
  useSelectedProvider: () => 'ollama',
};

// Stub useReferralDraft to control `draft` and `generate` deterministically.
export const referralDraftModule: Record<string, unknown> = {
  readReferralSeed: () => null,
  useReferralDraft: () => ({
    draft: stub.draft,
    generating: false,
    error: null,
    generate: mockGenerate,
    improve: mockImprove,
    abort: vi.fn(),
    canGenerate: true,
    // save()'s onSuccess calls reset() (the add-another flow) — stub it so the
    // success path doesn't throw on an undefined.
    reset: vi.fn(),
  }),
};

// The list is tested in its own file; stub it to avoid double-rendering issues.
export const referralListModule = { ReferralList: () => null };

// ModalShell manages focus traps and portals — render children directly.
export const uiModule = (actual: typeof AjhUi) =>
  ({
    ...actual,
    ModalShell: ({ children }: { children: React.ReactNode }) => (
      <div data-testid={TEST_IDS.documents.modalShell}>{children}</div>
    ),
  }) as unknown as typeof AjhUi;
