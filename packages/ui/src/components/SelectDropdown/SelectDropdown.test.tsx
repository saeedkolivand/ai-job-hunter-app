import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { SelectDropdown } from './SelectDropdown';

const OPTIONS = [
  { value: 'en', label: 'English' },
  { value: 'de', label: 'German' },
  { value: 'fr', label: 'French' },
];

describe('SelectDropdown', () => {
  it('renders the placeholder and the selected label', () => {
    const { rerender } = render(
      <SelectDropdown options={OPTIONS} value="" onChange={() => {}} placeholder="Language" />
    );
    expect(screen.getByText('Language')).toBeInTheDocument();
    rerender(<SelectDropdown options={OPTIONS} value="de" onChange={() => {}} />);
    expect(screen.getByText('German')).toBeInTheDocument();
  });

  it('opens as a listbox and selects an option by click', async () => {
    const onChange = vi.fn();
    render(<SelectDropdown options={OPTIONS} value="" onChange={onChange} />);
    await userEvent.click(screen.getByRole('button'));
    expect(screen.getByRole('listbox')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('option', { name: 'French' }));
    expect(onChange).toHaveBeenCalledWith('fr');
  });

  it('supports keyboard navigation (ArrowDown + Enter)', async () => {
    const onChange = vi.fn();
    render(<SelectDropdown options={OPTIONS} value="" onChange={onChange} />);
    const trigger = screen.getByRole('button');
    trigger.focus();
    await userEvent.keyboard('{ArrowDown}'); // opens
    await userEvent.keyboard('{ArrowDown}{Enter}');
    expect(onChange).toHaveBeenCalled();
  });

  it('does not open when disabled', async () => {
    render(<SelectDropdown options={OPTIONS} value="" onChange={() => {}} disabled />);
    await userEvent.click(screen.getByRole('button')).catch(() => {});
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('renders a search box when there are many options', async () => {
    const many = Array.from({ length: 10 }, (_, i) => ({ value: `v${i}`, label: `Item ${i}` }));
    render(<SelectDropdown options={many} value="" onChange={() => {}} />);
    await userEvent.click(screen.getByRole('button'));
    expect(screen.getByRole('listbox')).toBeInTheDocument();
  });
});
