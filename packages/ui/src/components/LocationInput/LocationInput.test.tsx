import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { LocationInput } from './LocationInput';

const noSuggestions = vi.fn().mockResolvedValue([]);

// Two geocoded suggestions used by keyboard-nav tests
const TWO_SUGGESTIONS = [
  { display: 'Berlin, Germany', lat: 52.52, lon: 13.4 },
  { display: 'Hamburg, Germany', lat: 53.55, lon: 9.99 },
];
const twoSuggestions = vi.fn().mockResolvedValue(TWO_SUGGESTIONS);

describe('LocationInput', () => {
  it('shows the placeholder when empty and the value otherwise', () => {
    const { rerender } = render(
      <LocationInput value="" onChange={() => {}} placeholder="Any location" />
    );
    expect(screen.getByText('Any location')).toBeInTheDocument();
    rerender(<LocationInput value="Berlin" onChange={() => {}} />);
    expect(screen.getByText('Berlin')).toBeInTheDocument();
  });

  it('clears the value via the clear affordance', () => {
    const onChange = vi.fn();
    render(<LocationInput value="Berlin" onChange={onChange} />);
    const clear = screen.getByRole('button', { name: '' });
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
    await act(async () => {
      await new Promise((r) => setTimeout(r, 350));
    });

    // ArrowDown once → activeIndex 0 (first suggestion: Berlin)
    // ArrowDown again → activeIndex 1 (second suggestion: Hamburg)
    fireEvent.keyDown(search, { key: 'ArrowDown' });
    fireEvent.keyDown(search, { key: 'ArrowDown' });
    fireEvent.keyDown(search, { key: 'Enter' });

    expect(onChange).toHaveBeenCalledWith('Hamburg, Germany');
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
    await act(async () => {
      await new Promise((r) => setTimeout(r, 350));
    });

    // Navigate to first suggestion and confirm
    fireEvent.keyDown(search, { key: 'ArrowDown' });
    fireEvent.keyDown(search, { key: 'Enter' });

    expect(onSelectSuggestion).toHaveBeenCalledWith(TWO_SUGGESTIONS[0]);
  });

  it('onSelectSuggestion is called with { display: "" } on clear', () => {
    const onChange = vi.fn();
    const onSelectSuggestion = vi.fn();
    render(
      <LocationInput value="Berlin" onChange={onChange} onSelectSuggestion={onSelectSuggestion} />
    );
    const clear = screen.getByRole('button', { name: '' });
    fireEvent.click(clear);
    expect(onSelectSuggestion).toHaveBeenCalledWith({ display: '' });
  });
});
