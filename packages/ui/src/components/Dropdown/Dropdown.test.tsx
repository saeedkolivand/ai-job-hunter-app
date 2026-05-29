import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Dropdown, type DropdownOption } from './Dropdown';

const OPTIONS: DropdownOption[] = [
  { value: 'a', label: 'Apple' },
  { value: 'b', label: 'Banana', meta: 'fruit' },
  { value: 'c', label: 'Cherry', section: 'Stone fruit' },
];

describe('Dropdown', () => {
  it('shows the placeholder when nothing is selected', () => {
    render(<Dropdown options={OPTIONS} value="" onChange={() => {}} placeholder="Pick one" />);
    expect(screen.getByText('Pick one')).toBeInTheDocument();
  });

  it('shows the selected option label', () => {
    render(<Dropdown options={OPTIONS} value="b" onChange={() => {}} />);
    expect(screen.getByText('Banana')).toBeInTheDocument();
  });

  it('opens on click and selects an option', async () => {
    const onChange = vi.fn();
    render(<Dropdown options={OPTIONS} value="" onChange={onChange} />);
    await userEvent.click(screen.getByRole('button'));
    await userEvent.click(screen.getByText('Cherry'));
    expect(onChange).toHaveBeenCalledWith('c');
  });

  it('filters options via the search box', async () => {
    const many: DropdownOption[] = Array.from({ length: 8 }, (_, i) => ({
      value: String(i),
      label: `Option ${i}`,
    }));
    render(<Dropdown options={many} value="" onChange={() => {}} searchable />);
    await userEvent.click(screen.getByRole('button'));
    await userEvent.type(screen.getByPlaceholderText('Search…'), 'Option 3');
    expect(screen.getByText('Option 3')).toBeInTheDocument();
    expect(screen.queryByText('Option 5')).not.toBeInTheDocument();
  });

  it('shows a "No results" message when nothing matches', async () => {
    render(<Dropdown options={OPTIONS} value="" onChange={() => {}} searchable />);
    await userEvent.click(screen.getByRole('button'));
    await userEvent.type(screen.getByPlaceholderText('Search…'), 'zzz');
    expect(screen.getByText('No results')).toBeInTheDocument();
  });

  it('does not open when disabled', async () => {
    render(<Dropdown options={OPTIONS} value="" onChange={() => {}} disabled />);
    await userEvent.click(screen.getByRole('button')).catch(() => {});
    expect(screen.queryByText('Apple')).not.toBeInTheDocument();
  });
});
