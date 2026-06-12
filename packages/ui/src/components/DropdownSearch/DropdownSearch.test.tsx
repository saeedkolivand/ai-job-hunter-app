import { createRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { DropdownSearch } from './DropdownSearch';

function renderSearch(overrides: Partial<Parameters<typeof DropdownSearch>[0]> = {}) {
  const searchRef = createRef<HTMLInputElement>();
  const props = {
    search: '',
    setSearch: vi.fn(),
    searchRef,
    ...overrides,
  };
  const result = render(<DropdownSearch {...props} />);
  return { ...result, searchRef, setSearch: props.setSearch as ReturnType<typeof vi.fn> };
}

describe('DropdownSearch', () => {
  describe('clear button', () => {
    it('renders the Clear search button when onClear is provided and search is non-empty', () => {
      renderSearch({ search: 'hello', onClear: vi.fn() });
      expect(screen.getByRole('button', { name: 'Clear search' })).toBeInTheDocument();
    });

    it('fires onClear when the clear button is clicked', async () => {
      const onClear = vi.fn();
      renderSearch({ search: 'hello', onClear });
      await userEvent.click(screen.getByRole('button', { name: 'Clear search' }));
      expect(onClear).toHaveBeenCalledTimes(1);
    });

    it('does NOT render the clear button when onClear is provided but search is empty', () => {
      renderSearch({ search: '', onClear: vi.fn() });
      expect(screen.queryByRole('button', { name: 'Clear search' })).not.toBeInTheDocument();
    });

    it('does NOT render the clear button when search is non-empty but onClear is absent', () => {
      renderSearch({ search: 'hello' });
      expect(screen.queryByRole('button', { name: 'Clear search' })).not.toBeInTheDocument();
    });
  });

  describe('onKeyDown forwarding', () => {
    it('calls onKeyDown when a key is pressed in the input', () => {
      const onKeyDown = vi.fn();
      renderSearch({ onKeyDown });
      const input = screen.getByRole('textbox');
      fireEvent.keyDown(input, { key: 'Enter' });
      expect(onKeyDown).toHaveBeenCalledTimes(1);
      expect(onKeyDown.mock.calls[0]?.[0]?.key).toBe('Enter');
    });
  });

  describe('custom placeholder', () => {
    it('shows the default placeholder when no placeholder prop is given', () => {
      renderSearch();
      expect(screen.getByPlaceholderText('Search…')).toBeInTheDocument();
    });

    it('shows a custom placeholder when one is provided', () => {
      renderSearch({ placeholder: 'Find a city…' });
      expect(screen.getByPlaceholderText('Find a city…')).toBeInTheDocument();
    });
  });
});
