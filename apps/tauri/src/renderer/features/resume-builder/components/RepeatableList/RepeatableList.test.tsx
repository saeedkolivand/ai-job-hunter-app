import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Input } from '@ajh/ui';

import { RepeatableList } from './index';

interface Item {
  value: string;
}

function setup(items: Item[]) {
  const onChange = vi.fn();
  render(
    <RepeatableList<Item>
      items={items}
      onChange={onChange}
      blank={() => ({ value: '' })}
      addLabel="Add item"
      removeLabel="Remove item"
      emptyLabel="Nothing yet"
      render={(item, set) => (
        <Input
          aria-label="value"
          value={item.value}
          onChange={(e) => set({ value: e.target.value })}
        />
      )}
    />
  );
  return { onChange };
}

describe('RepeatableList', () => {
  it('shows the empty label when there are no items', () => {
    setup([]);
    expect(screen.getByText('Nothing yet')).toBeInTheDocument();
  });

  it('appends a blank entry on add', async () => {
    const user = userEvent.setup();
    const { onChange } = setup([{ value: 'a' }]);
    await user.click(screen.getByRole('button', { name: 'Add item' }));
    expect(onChange).toHaveBeenCalledWith([{ value: 'a' }, { value: '' }]);
  });

  it('removes the targeted entry', async () => {
    const user = userEvent.setup();
    const { onChange } = setup([{ value: 'a' }, { value: 'b' }]);
    const removeButtons = screen.getAllByRole('button', { name: 'Remove item' });
    const second = removeButtons[1];
    expect(second).toBeDefined();
    if (second) await user.click(second);
    expect(onChange).toHaveBeenCalledWith([{ value: 'a' }]);
  });

  it('patches a single entry immutably on edit', async () => {
    const user = userEvent.setup();
    const { onChange } = setup([{ value: '' }]);
    await user.type(screen.getByLabelText('value'), 'x');
    expect(onChange).toHaveBeenLastCalledWith([{ value: 'x' }]);
  });
});
