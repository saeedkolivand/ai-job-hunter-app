import { useRef, useState } from 'react';
import type { Meta, StoryObj } from '@storybook/react-vite';

import { DropdownSearch } from './DropdownSearch';

const meta = {
  component: DropdownSearch,
  tags: ['autodocs'],
  parameters: { layout: 'centered' },
} satisfies Meta<typeof DropdownSearch>;
export default meta;
type Story = StoryObj<typeof DropdownSearch>;

function Demo() {
  const [search, setSearch] = useState('');
  const searchRef = useRef<HTMLInputElement>(null);
  return (
    <div className="dropdown-surface w-64 rounded-xl">
      <DropdownSearch search={search} setSearch={setSearch} searchRef={searchRef} />
    </div>
  );
}

export const Default: Story = { render: () => <Demo /> };
