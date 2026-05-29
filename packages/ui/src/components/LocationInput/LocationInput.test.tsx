import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { LocationInput } from './LocationInput';

const noSuggestions = vi.fn().mockResolvedValue([]);

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
});
