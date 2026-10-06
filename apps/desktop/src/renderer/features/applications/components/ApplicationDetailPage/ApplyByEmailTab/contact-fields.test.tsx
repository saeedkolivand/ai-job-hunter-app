/**
 * ApplyByEmailTab — canonical contact binding, rejected writes, synced buffers
 *
 * Shared mocks + fixtures live in ./test-support.
 */

import { describe, expect, it } from 'vitest';
import { fireEvent, screen } from '@testing-library/react';

import {
  ApplyByEmailTab,
  makeApp,
  NO_GENERATIONS,
  renderTab,
  updateApplicationMutate,
} from './test-support';

// ── Canonical contact binding ────────────────────────────────────────────────
//
// The email recipient IS the application's primary contact. These fields must
// read AND write `contactName` / `contactEmail` — the deprecated
// `recipientName` / `recipientEmail` aliases are never touched by this surface.

describe('ApplyByEmailTab — canonical contact binding', () => {
  it('seeds the recipient fields from contactName / contactEmail', () => {
    renderTab({ contactName: 'Dana Doe', contactEmail: 'dana@acme.com' });

    // Generic form (not an `as` cast): eslint's no-unnecessary-type-assertion
    // strips the cast, but `tsc` still needs the input type for `.value`.
    expect(
      screen.getByLabelText<HTMLInputElement>('applications.detail.email.recipientNameLabel').value
    ).toBe('Dana Doe');
    expect(
      screen.getByLabelText<HTMLInputElement>('applications.detail.email.recipientEmailLabel').value
    ).toBe('dana@acme.com');
  });

  it('persists the name to contactName (not the deprecated recipientName alias)', () => {
    renderTab();

    const field = screen.getByLabelText('applications.detail.email.recipientNameLabel');
    fireEvent.change(field, { target: { value: '  Dana Doe  ' } });
    fireEvent.blur(field);

    expect(updateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(updateApplicationMutate.mock.calls[0]?.[0]).toEqual({
      id: 'app-1',
      contactName: 'Dana Doe',
    });
  });

  it('persists the email to contactEmail (not the deprecated recipientEmail alias)', () => {
    renderTab();

    const field = screen.getByLabelText('applications.detail.email.recipientEmailLabel');
    fireEvent.change(field, { target: { value: 'dana@acme.com' } });
    fireEvent.blur(field);

    expect(updateApplicationMutate).toHaveBeenCalledTimes(1);
    expect(updateApplicationMutate.mock.calls[0]?.[0]).toEqual({
      id: 'app-1',
      contactEmail: 'dana@acme.com',
    });
  });

  it('does NOT write when the value is unchanged from the stored contact', () => {
    renderTab({ contactEmail: 'dana@acme.com' });

    fireEvent.blur(screen.getByLabelText('applications.detail.email.recipientEmailLabel'));
    expect(updateApplicationMutate).not.toHaveBeenCalled();
  });

  it('states the shared-contact consequence ONCE, referenced by both fields', () => {
    renderTab();

    // Rendered once — not duplicated under each field.
    const hints = screen.getAllByText('applications.detail.email.recipientHint');
    expect(hints).toHaveLength(1);

    // …and both fields point at that same node.
    const hintId = hints[0]?.getAttribute('id');
    expect(hintId).toBeTruthy();
    for (const label of ['recipientNameLabel', 'recipientEmailLabel']) {
      const field = screen.getByLabelText(`applications.detail.email.${label}`);
      expect(field.getAttribute('aria-describedby')).toBe(hintId);
    }
  });
});

// ── Server-rejected contact writes ───────────────────────────────────────────
//
// `validate_contact_name` / `validate_recipient_email` reject control chars,
// over-length values and malformed addresses by returning `{ error }` — they do
// NOT throw. Without surfacing, the field keeps showing text that was never
// stored and the mailto button silently targets the stale address.

describe('ApplyByEmailTab — rejected contact writes', () => {
  /** Makes the next update resolve as a rejection (or an acceptance). */
  const respondWith = (error?: string) =>
    updateApplicationMutate.mockImplementation(
      (_vars: unknown, options?: { onSuccess?: (data: { error?: string }) => void }) => {
        options?.onSuccess?.(error ? { error } : {});
      }
    );

  const blurName = (value: string) => {
    const field = screen.getByLabelText('applications.detail.email.recipientNameLabel');
    fireEvent.change(field, { target: { value } });
    fireEvent.blur(field);
    return field;
  };

  const blurEmail = (value: string) => {
    const field = screen.getByLabelText('applications.detail.email.recipientEmailLabel');
    fireEvent.change(field, { target: { value } });
    fireEvent.blur(field);
    return field;
  };

  it('surfaces a rejected NAME write as an alert', () => {
    respondWith('invalid contact name');
    renderTab();

    blurName('DanaDoe');

    expect(screen.getByRole('alert')).toHaveTextContent('applications.detail.contactSaveError');
  });

  it('surfaces a rejected EMAIL write as an alert', () => {
    respondWith('invalid email');
    renderTab();

    blurEmail('not-an-email');

    expect(screen.getByRole('alert')).toHaveTextContent('applications.detail.email.emailInvalid');
  });

  it('shows no alert when either write is accepted', () => {
    respondWith(undefined);
    renderTab();

    blurName('Dana Doe');
    blurEmail('dana@acme.com');

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('clears a stale rejection once the user edits the field again', () => {
    respondWith('invalid email');
    renderTab();

    const field = blurEmail('not-an-email');
    expect(screen.getByRole('alert')).toBeInTheDocument();

    fireEvent.change(field, { target: { value: 'dana@acme.com' } });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('a later ACCEPTED email write clears the earlier rejection', () => {
    respondWith('invalid email');
    renderTab();
    blurEmail('not-an-email');
    expect(screen.getByRole('alert')).toBeInTheDocument();

    respondWith(undefined);
    blurEmail('dana@acme.com');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  // The rejected value must not be presented as the stored contact: the mailto
  // link is built from the field, so a silent failure mails the wrong person.
  it('keeps the alert visible while the rejected text is still on screen', () => {
    respondWith('invalid email');
    renderTab();

    blurEmail('not-an-email');

    expect(
      screen.getByLabelText<HTMLInputElement>('applications.detail.email.recipientEmailLabel').value
    ).toBe('not-an-email');
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });
});

// ── Canonical pair re-seeds from the sibling surface ─────────────────────────

describe('ApplyByEmailTab — synced contact buffers', () => {
  it('adopts a contact written by the Overview tab (no remount)', () => {
    const { rerender } = renderTab({ contactName: '', contactEmail: '' });
    expect(
      screen.getByLabelText<HTMLInputElement>('applications.detail.email.recipientNameLabel').value
    ).toBe('');

    // The Overview card saved the contact; the record refetches into this tab.
    rerender(
      <ApplyByEmailTab
        application={makeApp({ contactName: 'Rita Recruiter', contactEmail: 'rita@acme.com' })}
        matchingGenerations={NO_GENERATIONS}
      />
    );

    expect(
      screen.getByLabelText<HTMLInputElement>('applications.detail.email.recipientNameLabel').value
    ).toBe('Rita Recruiter');
    expect(
      screen.getByLabelText<HTMLInputElement>('applications.detail.email.recipientEmailLabel').value
    ).toBe('rita@acme.com');
  });

  // Mirrors the Overview wipe-regression test: after adopting a value written
  // elsewhere, blurring must NOT persist anything — the buffer already agrees
  // with the record, so a write here would be a no-op at best and a stale
  // overwrite at worst.
  it('adopts an out-of-band contactEmail and does not re-persist it on blur', () => {
    const { rerender } = renderTab({ contactEmail: '' });

    rerender(
      <ApplyByEmailTab
        application={makeApp({ contactEmail: 'rita@acme.com', updatedAt: 2000 })}
        matchingGenerations={NO_GENERATIONS}
      />
    );

    const field = screen.getByLabelText<HTMLInputElement>(
      'applications.detail.email.recipientEmailLabel'
    );
    expect(field.value).toBe('rita@acme.com');

    fireEvent.blur(field);
    expect(updateApplicationMutate).not.toHaveBeenCalled();
  });

  it('does NOT clobber an uncommitted edit when an unrelated field changes', () => {
    const { rerender } = renderTab({ contactName: '', contactEmail: '' });

    const nameField = screen.getByLabelText('applications.detail.email.recipientNameLabel');
    fireEvent.change(nameField, { target: { value: 'typing…' } });

    // A status/note write bumps the record; the contact columns are untouched.
    rerender(
      <ApplyByEmailTab
        application={makeApp({ contactName: '', contactEmail: '', updatedAt: 9999 })}
        matchingGenerations={NO_GENERATIONS}
      />
    );

    expect(
      screen.getByLabelText<HTMLInputElement>('applications.detail.email.recipientNameLabel').value
    ).toBe('typing…');
  });
});
