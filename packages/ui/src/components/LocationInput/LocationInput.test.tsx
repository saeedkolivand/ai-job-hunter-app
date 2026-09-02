import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { LocationInput } from './LocationInput';

const noSuggestions = vi.fn().mockResolvedValue([]);

// Two geocoded suggestions used by keyboard-nav tests. Named individually so the
// tests can wait on / assert a concrete one without indexing (which is
// `| undefined` under noUncheckedIndexedAccess).
const BERLIN = { display: 'Berlin, Germany', lat: 52.52, lon: 13.4 };
const HAMBURG = { display: 'Hamburg, Germany', lat: 53.55, lon: 9.99 };
const TWO_SUGGESTIONS = [BERLIN, HAMBURG];
const twoSuggestions = vi.fn().mockResolvedValue(TWO_SUGGESTIONS);

describe('LocationInput', () => {
  it('shows the placeholder when empty and the value otherwise', () => {
    const { rerender } = render(
      <LocationInput
        value=""
        onChange={() => {}}
        onFetchSuggestions={noSuggestions}
        placeholder="Any location"
      />
    );
    expect(screen.getByText('Any location')).toBeInTheDocument();
    rerender(
      <LocationInput value="Berlin" onChange={() => {}} onFetchSuggestions={noSuggestions} />
    );
    expect(screen.getByText('Berlin')).toBeInTheDocument();
  });

  it('clears the value via the clear affordance', () => {
    const onChange = vi.fn();
    render(<LocationInput value="Berlin" onChange={onChange} onFetchSuggestions={noSuggestions} />);
    const clear = screen.getByRole('button', { name: 'Clear' });
    fireEvent.click(clear);
    expect(onChange).toHaveBeenCalledWith('');
  });

  it('opens the dropdown and accepts a custom typed location on Enter', async () => {
    const onChange = vi.fn();
    render(<LocationInput value="" onChange={onChange} onFetchSuggestions={noSuggestions} />);
    await userEvent.click(screen.getByRole('button'));
    const search = await screen.findByPlaceholderText('Search city or postcode…');
    await userEvent.type(search, 'Remote');
    fireEvent.keyDown(search, { key: 'Enter' });
    expect(onChange).toHaveBeenCalledWith('Remote');
  });

  it('closes on Escape', async () => {
    render(<LocationInput value="" onChange={() => {}} onFetchSuggestions={noSuggestions} />);
    await userEvent.click(screen.getByRole('button'));
    const search = await screen.findByPlaceholderText('Search city or postcode…');
    fireEvent.keyDown(search, { key: 'Escape' });
    expect(screen.queryByPlaceholderText('Search city or postcode…')).not.toBeInTheDocument();
  });

  // ── New coverage flagged by testing-reviewer ──────────────────────────────

  it('ArrowDown twice through two suggestions then Enter selects the second suggestion', async () => {
    const onChange = vi.fn();
    render(<LocationInput value="" onChange={onChange} onFetchSuggestions={twoSuggestions} />);
    await userEvent.click(screen.getByRole('button'));
    const search = await screen.findByPlaceholderText('Search city or postcode…');

    // Type enough characters to trigger the debounced fetch (>=2 chars)
    fireEvent.change(search, { target: { value: 'Be' } });

    // Wait for the debounced suggestions to arrive
    // Render-aware wait, not a fixed sleep: block until a CONCRETE suggestion from
    // TWO_SUGGESTIONS is on screen. A timer that expired before the debounced fetch
    // resolved would leave `suggestions` empty, and the ArrowDown/Enter below would
    // silently take the free-text fallback path instead of the keyboard-nav path
    // under test.
    await screen.findByText(BERLIN.display);

    // ArrowDown once → activeIndex 0 (first suggestion: Berlin)
    // ArrowDown again → activeIndex 1 (second suggestion: Hamburg)
    fireEvent.keyDown(search, { key: 'ArrowDown' });
    fireEvent.keyDown(search, { key: 'ArrowDown' });
    fireEvent.keyDown(search, { key: 'Enter' });

    expect(onChange).toHaveBeenCalledWith(HAMBURG.display);
  });

  it('onSelectSuggestion is called with the full structured object on suggestion pick', async () => {
    const onChange = vi.fn();
    const onSelectSuggestion = vi.fn();
    render(
      <LocationInput
        value=""
        onChange={onChange}
        onFetchSuggestions={twoSuggestions}
        onSelectSuggestion={onSelectSuggestion}
      />
    );
    await userEvent.click(screen.getByRole('button'));
    const search = await screen.findByPlaceholderText('Search city or postcode…');

    fireEvent.change(search, { target: { value: 'Be' } });
    // Render-aware wait, not a fixed sleep: block until a CONCRETE suggestion from
    // TWO_SUGGESTIONS is on screen. A timer that expired before the debounced fetch
    // resolved would leave `suggestions` empty, and the ArrowDown/Enter below would
    // silently take the free-text fallback path instead of the keyboard-nav path
    // under test.
    await screen.findByText(BERLIN.display);

    // Navigate to first suggestion and confirm
    fireEvent.keyDown(search, { key: 'ArrowDown' });
    fireEvent.keyDown(search, { key: 'Enter' });

    expect(onSelectSuggestion).toHaveBeenCalledWith(BERLIN);
  });

  it('fires onChange BEFORE onSelectSuggestion when a suggestion is picked', async () => {
    // Load-bearing ORDER, pinned at the source. Consumers (ScrapeFilters, the
    // autopilot StepTarget) wire `onChange` to "location edited → clear the
    // resolved countryCode" and `onSelectSuggestion` to "write the picked
    // suggestion's countryCode". Reversing the two calls inside `select()` would
    // let the onChange handler wipe the country the pick just resolved — a
    // silent geo-targeting regression with every consumer test still green
    // (their doubles copy the order rather than depend on it).
    const onChange = vi.fn();
    const onSelectSuggestion = vi.fn();
    render(
      <LocationInput
        value=""
        onChange={onChange}
        onFetchSuggestions={twoSuggestions}
        onSelectSuggestion={onSelectSuggestion}
      />
    );
    await userEvent.click(screen.getByRole('button'));
    const search = await screen.findByPlaceholderText('Search city or postcode…');

    fireEvent.change(search, { target: { value: 'Be' } });
    // Render-aware wait, not a fixed sleep: block until a CONCRETE suggestion from
    // TWO_SUGGESTIONS is on screen. A timer that expired before the debounced fetch
    // resolved would leave `suggestions` empty, and the ArrowDown/Enter below would
    // silently take the free-text fallback path instead of the keyboard-nav path
    // under test.
    await screen.findByText(BERLIN.display);

    fireEvent.keyDown(search, { key: 'ArrowDown' });
    fireEvent.keyDown(search, { key: 'Enter' });

    // vitest stamps every mock call with a global monotonic sequence number, so
    // this compares the two callbacks across mocks (a per-mock call index cannot).
    const changeOrder = onChange.mock.invocationCallOrder;
    const selectOrder = onSelectSuggestion.mock.invocationCallOrder;
    expect(changeOrder).not.toHaveLength(0);
    expect(selectOrder).not.toHaveLength(0);
    expect(Math.min(...changeOrder)).toBeLessThan(Math.min(...selectOrder));
  });

  it('onSelectSuggestion is called with { display: "" } on clear', () => {
    const onChange = vi.fn();
    const onSelectSuggestion = vi.fn();
    render(
      <LocationInput
        value="Berlin"
        onChange={onChange}
        onFetchSuggestions={noSuggestions}
        onSelectSuggestion={onSelectSuggestion}
      />
    );
    const clear = screen.getByRole('button', { name: 'Clear' });
    fireEvent.click(clear);
    expect(onSelectSuggestion).toHaveBeenCalledWith({ display: '' });
  });
});

describe('LocationInput — clear affordance', () => {
  it('the clear button is a keyboard-reachable SIBLING of the field, not nested inside it', async () => {
    const onChange = vi.fn();
    render(<LocationInput value="Berlin" onChange={onChange} onFetchSuggestions={noSuggestions} />);

    const field = screen.getByRole('button', { name: 'Berlin' });
    const clear = screen.getByRole('button', { name: 'Clear' });
    // It used to be a role="button" <span> INSIDE the field <button>: invalid
    // markup, so the browser never put it in the tab order.
    expect(field.contains(clear)).toBe(false);

    await userEvent.tab(); // field trigger
    await userEvent.tab(); // clear
    expect(clear).toHaveFocus();

    await userEvent.keyboard('{Enter}');
    expect(onChange).toHaveBeenCalledWith('');
  });

  it('names the clear button, and lets the consumer localize that name', () => {
    render(
      <LocationInput
        value="Berlin"
        onChange={() => {}}
        onFetchSuggestions={noSuggestions}
        clearLabel="Ort löschen"
      />
    );
    expect(screen.getByRole('button', { name: 'Ort löschen' })).toBeInTheDocument();
  });

  it('renders no clear button when the field is empty or disabled', () => {
    const { rerender } = render(
      <LocationInput value="" onChange={() => {}} onFetchSuggestions={noSuggestions} />
    );
    expect(screen.queryByRole('button', { name: 'Clear' })).toBeNull();
    rerender(
      <LocationInput
        value="Berlin"
        disabled
        onChange={() => {}}
        onFetchSuggestions={noSuggestions}
      />
    );
    expect(screen.queryByRole('button', { name: 'Clear' })).toBeNull();
  });
});
