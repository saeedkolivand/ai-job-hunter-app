import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';

import type { ContactFieldConflict, ContactProfile } from '@ajh/shared';
import type * as AjhUi from '@ajh/ui';

import { ContactConflictModal } from './index';

// ── Module mocks ──────────────────────────────────────────────────────────────
// Vitest hoists vi.mock() calls before any imports at runtime, so the import
// order above has no effect on mock resolution.

vi.mock('@/lib/i18n', () => ({
  useTranslation: () => ({ t: (k: string, _opts?: Record<string, unknown>) => k }),
}));

// useNotification: return a spy; tests assert it is/isn't called.
const mockNotify = vi.fn<(message: string, variant: string) => void>();

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useNotification: () => mockNotify,
  };
});

// useContactProfile returns a fixed base profile with a location.byLang.
// useSaveContactProfile exposes a typed mutateAsync spy.
const mockMutateAsync = vi.fn<(profile: ContactProfile) => Promise<void>>();

vi.mock('@/services', () => ({
  useContactProfile: (): { data: ContactProfile } => ({
    data: BASE_PROFILE,
  }),
  useSaveContactProfile: () => ({
    mutateAsync: mockMutateAsync,
    isPending: false,
  }),
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

const BASE_PROFILE: ContactProfile = {
  fullName: 'Alex Carter',
  email: 'alex@example.com',
  phone: '+31 6 12345678',
  location: {
    default: 'Amsterdam, Netherlands',
    byLang: { de: 'Amsterdam, Niederlande' },
  },
  linkedin: 'https://linkedin.com/in/alexcarter',
  github: 'https://github.com/alexcarter',
  website: 'https://alex.dev',
};

// NOTE: EMAIL_CONFLICT.current is intentionally DIFFERENT from BASE_PROFILE.email
// so the keep-mine test is observable: if the component ignores `fields[field].value`
// and blindly spreads BASE_PROFILE, saved.email would be 'alex@example.com' (the
// base profile value) rather than 'old.alex@example.com' (the conflict's current).
const EMAIL_CONFLICT: ContactFieldConflict = {
  field: 'email',
  current: 'old.alex@example.com',
  suggested: 'alex.carter@resume.com',
};

const PHONE_CONFLICT: ContactFieldConflict = {
  field: 'phone',
  current: '+31 6 12345678',
  suggested: '+1 (800) 555-0199',
};

const LOCATION_CONFLICT: ContactFieldConflict = {
  field: 'location',
  current: 'Amsterdam, Netherlands',
  suggested: 'Berlin, Germany',
};

// ── Helpers ───────────────────────────────────────────────────────────────────

interface RenderOpts {
  conflicts?: ContactFieldConflict[];
  onClose?: () => void;
  onResolved?: () => void;
}

function renderModal({
  conflicts = [EMAIL_CONFLICT],
  onClose = vi.fn<() => void>(),
  onResolved = vi.fn<() => void>(),
}: RenderOpts = {}) {
  render(
    <ContactConflictModal
      open={true}
      conflicts={conflicts}
      onClose={onClose}
      onResolved={onResolved}
    />
  );
  return { onClose, onResolved };
}

/** Click the Save button and wait for async save to settle. */
async function clickSave() {
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /contactConflict\.save/i }));
  });
}

/** Find the SegmentedControl radio for a given i18n label key. */
function getRadio(labelKey: string) {
  return screen.getByRole('radio', { name: labelKey });
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ContactConflictModal — F2 contact-mismatch warning', () => {
  beforeEach(() => {
    mockMutateAsync.mockReset();
    mockNotify.mockReset();
    mockMutateAsync.mockResolvedValue(undefined);
  });

  // ── Rendering ──────────────────────────────────────────────────────────────

  it('renders one row per conflict, showing both current and suggested values', () => {
    renderModal({ conflicts: [EMAIL_CONFLICT, PHONE_CONFLICT] });

    // Both current values appear.
    expect(screen.getByText(EMAIL_CONFLICT.current)).toBeInTheDocument();
    expect(screen.getByText(PHONE_CONFLICT.current)).toBeInTheDocument();
    // Both suggested values appear.
    expect(screen.getByText(EMAIL_CONFLICT.suggested)).toBeInTheDocument();
    expect(screen.getByText(PHONE_CONFLICT.suggested)).toBeInTheDocument();
  });

  it('renders one editable Input per conflict, pre-seeded with the current value', () => {
    renderModal({ conflicts: [EMAIL_CONFLICT, PHONE_CONFLICT] });

    const inputs = screen.getAllByRole('textbox');
    const inputValues = inputs.map((el) => (el as HTMLInputElement).value);
    expect(inputValues).toContain(EMAIL_CONFLICT.current);
    expect(inputValues).toContain(PHONE_CONFLICT.current);
  });

  // ── Default keep-mine ──────────────────────────────────────────────────────

  it('default keep-mine: Save routes through fields[field].value seeded to conflict.current', async () => {
    // EMAIL_CONFLICT.current ('old.alex@example.com') differs from BASE_PROFILE.email
    // ('alex@example.com'). A no-op implementation that blindly spreads the base
    // profile would save BASE_PROFILE.email — this test catches that regression.
    renderModal({ conflicts: [EMAIL_CONFLICT] });

    // 'mine' radio is default (aria-checked=true on keep-mine option).
    const keepMineRadio = getRadio('settings.contactConflict.keepMine');
    expect(keepMineRadio).toHaveAttribute('aria-checked', 'true');

    await clickSave();

    await waitFor(() => expect(mockMutateAsync).toHaveBeenCalledTimes(1));
    const saved = mockMutateAsync.mock.calls[0]?.[0] as ContactProfile;

    // Must equal conflict.current — the value seeded into fields[email].value —
    // NOT BASE_PROFILE.email. Asserts the full computed payload shape too.
    expect(saved.email).toBe(EMAIL_CONFLICT.current);
    expect(saved.email).not.toBe(BASE_PROFILE.email);
    // Non-conflicting fields are preserved from the base profile.
    expect(saved.phone).toBe(BASE_PROFILE.phone);
    expect(saved.linkedin).toBe(BASE_PROFILE.linkedin);
    expect(saved.github).toBe(BASE_PROFILE.github);
    expect(saved.website).toBe(BASE_PROFILE.website);
  });

  // ── Switch to use-résumé ───────────────────────────────────────────────────

  it('switch to use-résumé: Save payload has the field set to conflict.suggested', async () => {
    renderModal({ conflicts: [EMAIL_CONFLICT] });

    // Switch to use-résumé.
    fireEvent.click(getRadio('settings.contactConflict.useResume'));
    await clickSave();

    await waitFor(() => expect(mockMutateAsync).toHaveBeenCalledTimes(1));
    const saved = mockMutateAsync.mock.calls[0]?.[0] as ContactProfile;
    expect(saved.email).toBe(EMAIL_CONFLICT.suggested);
  });

  // ── Edit the Input then use-résumé ────────────────────────────────────────

  it('editing the Input then choosing use-résumé: Save payload uses the EDITED value', async () => {
    renderModal({ conflicts: [EMAIL_CONFLICT] });

    // Switch to use-résumé first — seeds the input with suggested.
    fireEvent.click(getRadio('settings.contactConflict.useResume'));

    // Now hand-edit the Input to a custom value.
    const input = screen
      .getAllByRole('textbox')
      .find((el) => (el as HTMLInputElement).value === EMAIL_CONFLICT.suggested);
    if (!input) throw new Error('input seeded with suggested value not found');
    fireEvent.change(input, { target: { value: 'custom@edited.com' } });

    await clickSave();

    await waitFor(() => expect(mockMutateAsync).toHaveBeenCalledTimes(1));
    const saved = mockMutateAsync.mock.calls[0]?.[0] as ContactProfile;
    expect(saved.email).toBe('custom@edited.com');
  });

  // ── location field: byLang preserved ──────────────────────────────────────

  it('location use-résumé: payload location.default = suggested AND existing byLang preserved', async () => {
    renderModal({ conflicts: [LOCATION_CONFLICT] });

    fireEvent.click(getRadio('settings.contactConflict.useResume'));
    await clickSave();

    await waitFor(() => expect(mockMutateAsync).toHaveBeenCalledTimes(1));
    const saved = mockMutateAsync.mock.calls[0]?.[0] as ContactProfile;
    expect(saved.location?.default).toBe(LOCATION_CONFLICT.suggested);
    // byLang from BASE_PROFILE must be preserved intact.
    expect(saved.location?.byLang).toEqual(BASE_PROFILE.location?.byLang);
  });

  // ── Clearing the location Input ────────────────────────────────────────────

  it('clearing the location Input: location kept as current (no empty default written)', async () => {
    renderModal({ conflicts: [LOCATION_CONFLICT] });

    // Switch to use-résumé, then clear the input.
    fireEvent.click(getRadio('settings.contactConflict.useResume'));
    const input = screen
      .getAllByRole('textbox')
      .find((el) => (el as HTMLInputElement).value === LOCATION_CONFLICT.suggested);
    if (!input) throw new Error('location input not found');
    fireEvent.change(input, { target: { value: '' } });

    await clickSave();

    await waitFor(() => expect(mockMutateAsync).toHaveBeenCalledTimes(1));
    const saved = mockMutateAsync.mock.calls[0]?.[0] as ContactProfile;
    // An empty value must NOT write an empty-string default — location stays
    // as it was on the base profile.
    expect(saved.location?.default).toBe(BASE_PROFILE.location?.default);
  });

  // ── Dismiss / cancel ───────────────────────────────────────────────────────

  it('clicking the Dismiss button does not call mutateAsync', () => {
    const onClose = vi.fn<() => void>();
    renderModal({ conflicts: [EMAIL_CONFLICT], onClose });

    fireEvent.click(screen.getByRole('button', { name: /contactConflict\.dismiss/i }));

    expect(mockMutateAsync).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  // ── Multiple conflicts, partial resolution ─────────────────────────────────

  it('multiple conflicts: each field resolved independently', async () => {
    renderModal({ conflicts: [EMAIL_CONFLICT, PHONE_CONFLICT] });

    // Each SegmentedControl renders as a radiogroup. The conflicts array order
    // matches the DOM order: index 0 = email, index 1 = phone.
    const [emailGroup, phoneGroup] = screen.getAllByRole('radiogroup');
    if (!emailGroup || !phoneGroup) throw new Error('radiogroups not found');

    // For email: switch to use-résumé using a scoped query inside that group.
    fireEvent.click(
      within(emailGroup).getByRole('radio', { name: 'settings.contactConflict.useResume' })
    );
    // For phone: leave as keep-mine (already the default).

    await clickSave();

    await waitFor(() => expect(mockMutateAsync).toHaveBeenCalledTimes(1));
    const saved = mockMutateAsync.mock.calls[0]?.[0] as ContactProfile;
    expect(saved.email).toBe(EMAIL_CONFLICT.suggested);
    // phone must be the conflict.current value (keep-mine path via fields state).
    expect(saved.phone).toBe(PHONE_CONFLICT.current);
  });

  // ── onResolved callback ────────────────────────────────────────────────────

  it('Save calls onResolved and onClose after successful mutateAsync', async () => {
    const onResolved = vi.fn<() => void>();
    const onClose = vi.fn<() => void>();
    renderModal({ conflicts: [EMAIL_CONFLICT], onResolved, onClose });

    await clickSave();

    await waitFor(() => expect(onResolved).toHaveBeenCalledTimes(1));
    // Success path must close the modal.
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('Save does NOT call onResolved or onClose when mutateAsync rejects', async () => {
    mockMutateAsync.mockRejectedValue(new Error('save failed'));
    const onResolved = vi.fn<() => void>();
    const onClose = vi.fn<() => void>();
    renderModal({ conflicts: [EMAIL_CONFLICT], onResolved, onClose });

    await clickSave();

    await waitFor(() => expect(mockNotify).toHaveBeenCalled());
    // Error path must keep the modal open.
    expect(onResolved).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });
});
