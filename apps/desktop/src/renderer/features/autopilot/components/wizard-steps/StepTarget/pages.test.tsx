/**
 * StepTarget — page budget field, inert for an aggregator-only target
 *
 * Shared mocks + fixtures live in ./test-support.
 */

import { describe, expect, it } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { readProbe, renderStep } from './test-support';

// ── Page-budget field ───────────────────────────────────────────────────────
// The control used to be an "Items to scrape" count (1–500) that the save step
// divided by 25, so every value above 250 collapsed to the same 10 pages and
// silently did nothing. It is now the page budget itself, bounded to the
// backend's AutopilotTargetSchema range.

describe('StepTarget — page budget field', () => {
  // A pages-HONOURING board is selected throughout: the field is deliberately
  // inert (disabled) when the aggregator is the only target — see the
  // "inert page budget" describe below.
  const renderPagesStep = (overrides: Parameters<typeof renderStep>[0] = {}) =>
    renderStep({ boards: ['greenhouse'], ...overrides });

  it('bounds the control to the backend 1–10 page range and seeds the form value', () => {
    renderPagesStep({ pages: 2 });

    const input = screen.getByRole<HTMLInputElement>('spinbutton');
    expect(input).toHaveAttribute('min', '1');
    expect(input).toHaveAttribute('max', '10');
    expect(input.value).toBe('2');
    expect(input).toBeEnabled();
  });

  it('gives the control an accessible name via the label/htmlFor pair', () => {
    // WizardField renders its <label> as a SIBLING of the control, so without
    // matching htmlFor/id the input announces as "spinbutton, blank".
    renderPagesStep({ pages: 2 });

    expect(screen.getByLabelText('autopilot.wizard.target.pages')).toBe(
      screen.getByRole('spinbutton')
    );
  });

  it('marks the field invalid inline while a non-integer is still unblurred', async () => {
    // The blur-round below is not guaranteed to run: on WKWebView a click does
    // not focus the clicked button, so "Next" can be pressed without the input
    // ever blurring. The resolver then rejects `pages` and — before this wiring
    // — the only feedback was the wizard's generic "missing fields" banner,
    // with nothing marking the offending control for sighted or SR users.
    renderPagesStep({ pages: 2 });

    const input = screen.getByRole<HTMLInputElement>('spinbutton');
    fireEvent.change(input, { target: { value: '2.5' } });

    // The message is an i18n KEY resolved through WizardField's `error` prop
    // (t is mocked to identity here), never zod's raw English default.
    expect(await screen.findByText('autopilot.wizard.validation.pagesRange')).toBeInTheDocument();
    expect(input).toHaveAttribute('aria-invalid', 'true');
  });

  it('clears the inline error once the value is valid again', async () => {
    renderPagesStep({ pages: 2 });

    const input = screen.getByRole<HTMLInputElement>('spinbutton');
    fireEvent.change(input, { target: { value: '2.5' } });
    await screen.findByText('autopilot.wizard.validation.pagesRange');

    fireEvent.blur(input); // rounds to 3 → valid
    await waitFor(() => {
      expect(screen.queryByText('autopilot.wizard.validation.pagesRange')).toBeNull();
    });
    expect(input).toHaveAttribute('aria-invalid', 'false');
  });

  it('rounds a non-integer entry on blur so it can never reach the .int() schema', () => {
    // NumberField clamps to [min, max] on blur but never rounds. An unrounded
    // 2.5 fails the resolver, and the final Create button runs through
    // handleSubmit with no onInvalid branch — it would just do nothing.
    renderPagesStep({ pages: 2 });

    const input = screen.getByRole<HTMLInputElement>('spinbutton');
    fireEvent.change(input, { target: { value: '2.5' } });
    expect(readProbe().pages).toBe(2.5); // unrounded while the field is focused

    fireEvent.blur(input);
    expect(readProbe().pages).toBe(3);
    expect(input.value).toBe('3'); // the displayed buffer follows the rounded value
  });

  it('writes a typed page count straight into the form (no ×25 item detour)', async () => {
    const user = userEvent.setup();
    renderPagesStep({ pages: 2 });

    const input = screen.getByRole('spinbutton');
    await user.clear(input);
    await user.type(input, '7');

    expect(readProbe().pages).toBe(7);
  });

  it('clamps an over-range entry down to 10 on blur instead of accepting a no-op value', async () => {
    const user = userEvent.setup();
    renderPagesStep({ pages: 2 });

    const input = screen.getByRole('spinbutton');
    await user.clear(input);
    await user.type(input, '40');
    await user.tab();

    expect(readProbe().pages).toBe(10);
  });
});

// ── Inert page budget for an aggregator-only target (WS11) ──────────────────
// The aggregator board never reads `pages` — its providers page by the requested
// result AMOUNT instead (Adzuna's free tier is a daily call quota). An editable
// control that changes nothing is dishonest UI, so it is disabled — and ONLY when
// the aggregator is the sole target; every other board still honours the budget.

describe('StepTarget — page budget is inert for an aggregator-only target', () => {
  it('disables the control and swaps in the "not used" hint when aggregator is the only board', () => {
    renderStep({ boards: ['aggregator'], pages: 2 });

    expect(screen.getByRole('spinbutton')).toBeDisabled();
    expect(screen.getByText('autopilot.wizard.target.pagesAggregatorOnly')).toBeInTheDocument();
    // The generic per-board hint must be REPLACED, not shown alongside — it
    // claims the budget applies, which is exactly what is false here.
    expect(screen.queryByText('autopilot.wizard.target.pagesHint')).toBeNull();
  });

  it('keeps the control live for a MIXED selection that includes the aggregator', () => {
    // greenhouse honours the page budget, so the knob is real again.
    renderStep({ boards: ['aggregator', 'greenhouse'], pages: 2 });

    expect(screen.getByRole('spinbutton')).toBeEnabled();
    expect(screen.getByText('autopilot.wizard.target.pagesHint')).toBeInTheDocument();
    expect(screen.queryByText('autopilot.wizard.target.pagesAggregatorOnly')).toBeNull();
  });

  it('keeps the control live when the aggregator is not selected at all', () => {
    renderStep({ boards: ['greenhouse'], pages: 2 });

    expect(screen.getByRole('spinbutton')).toBeEnabled();
    expect(screen.getByText('autopilot.wizard.target.pagesHint')).toBeInTheDocument();
  });

  it('treats a duplicated aggregator id as an aggregator-ONLY target', () => {
    // `boards.length` would call ['aggregator','aggregator'] a MIXED selection and
    // leave a knob live that still changes nothing. The dedup'd set is the honest
    // count — same source `aggregatorSelected` reads.
    renderStep({ boards: ['aggregator', 'aggregator'], pages: 2 });

    expect(screen.getByRole('spinbutton')).toBeDisabled();
    expect(screen.getByText('autopilot.wizard.target.pagesAggregatorOnly')).toBeInTheDocument();
  });

  it('announces the disabled-reason hint to screen readers via aria-describedby', () => {
    // The hint is the ONLY explanation of why the control is dead. As an orphan
    // <span> next to the label it reached no SR user, so it is wired as the
    // control's description (not concatenated into its name).
    renderStep({ boards: ['aggregator'], pages: 2 });

    const input = screen.getByRole('spinbutton');
    const describedBy = input.getAttribute('aria-describedby');
    expect(describedBy).toBeTruthy();

    const hint = document.getElementById(describedBy ?? '');
    expect(hint).not.toBeNull();
    expect(hint).toHaveTextContent('autopilot.wizard.target.pagesAggregatorOnly');
  });

  it('preserves the stored page value while the control is disabled', async () => {
    // Disabling must not silently rewrite the saved budget: re-adding a
    // pages-honouring board has to bring the user's own number back.
    const user = userEvent.setup();
    renderStep({ boards: ['aggregator'], pages: 7 });

    expect(readProbe().pages).toBe(7);

    // Selecting a second board un-inerts the field with the value intact.
    await user.click(screen.getByRole('button', { name: 'jobs.boards.greenhouse' }));

    expect(screen.getByRole('spinbutton')).toBeEnabled();
    expect(readProbe().pages).toBe(7);
  });

  // ── Previously an UNFIXABLE block ────────────────────────────────────────
  // `pages` stays schema-validated while inert, so a persisted out-of-range or
  // non-integer value failed the resolver with the only control that could fix it
  // disabled: the wizard refused to advance and offered no way to comply.

  it.each([
    ['out of range (persisted 15)', 15, 10],
    ['below range (persisted 0)', 0, 1],
    ['non-integer (persisted 2.5)', 2.5, 3],
    ['not a number at all', Number.NaN, 2],
  ])('normalizes a %s pages value instead of blocking the wizard', async (_label, stored, want) => {
    renderStep({ boards: ['aggregator'], pages: stored });

    await waitFor(() => expect(readProbe().pages).toBe(want));
    // The actual regression: the resolver must accept the form again, so "Next"
    // is reachable. Asserting only the value would miss a normalization that
    // landed outside the schema range.
    await waitFor(() => expect(readProbe().isValid).toBe(true));
    expect(screen.queryByText('autopilot.wizard.validation.pagesRange')).toBeNull();
    // Still inert — normalizing is not the same as making the knob real.
    expect(screen.getByRole('spinbutton')).toBeDisabled();
  });

  it('hands the normalized value back as an editable number once a pages-aware board joins', async () => {
    const user = userEvent.setup();
    renderStep({ boards: ['aggregator'], pages: 15 });

    await waitFor(() => expect(readProbe().pages).toBe(10));

    await user.click(screen.getByRole('button', { name: 'jobs.boards.greenhouse' }));

    const input = screen.getByRole<HTMLInputElement>('spinbutton');
    expect(input).toBeEnabled();
    expect(input.value).toBe('10');
    expect(readProbe().isValid).toBe(true);
  });

  it('leaves an in-range persisted value untouched', async () => {
    // Normalization must only ever fire on a value the schema would REJECT —
    // silently rewriting a legitimate budget would be its own bug.
    renderStep({ boards: ['aggregator'], pages: 7 });

    await waitFor(() => expect(readProbe().isValid).toBe(true));
    expect(readProbe().pages).toBe(7);
  });
});
